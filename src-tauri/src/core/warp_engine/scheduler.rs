use super::wire::ParameterSet;
use crate::config::{
    warp_profiles::{TransportKind, WarpTransport},
    warp_selection::SelectionPolicy,
};
use anyhow::{Result, ensure};
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    time::Duration,
};
use tokio::time::Instant;

pub(crate) struct ScheduledBatch {
    pub(crate) token: u64,
    pub(crate) kind: TransportKind,
    pub(crate) parameter_name: String,
    pub(crate) transport: WarpTransport,
    pub(crate) ipv6: bool,
    pub(crate) initial: bool,
    pub(crate) targets: Vec<SocketAddr>,
    pub(crate) timeout_ms: u32,
    pub(crate) deadline: Instant,
}

struct Stream {
    kind: TransportKind,
    parameter_name: String,
    transport: WarpTransport,
    ipv6: bool,
    targets: VecDeque<SocketAddr>,
}
struct Active {
    stream: usize,
    initial: bool,
    started: Instant,
    deadline: Instant,
}

pub(crate) struct SearchSchedule {
    streams: Vec<Stream>,
    initial: VecDeque<usize>,
    expansion: VecDeque<usize>,
    active: HashMap<u64, Active>,
    spent: [Duration; 4],
    per_transport: Duration,
    discovery_deadline: Instant,
    next_token: u64,
}

fn slot(kind: TransportKind) -> usize {
    match kind {
        TransportKind::WireGuard => 0,
        TransportKind::Awg => 1,
        TransportKind::MasqueH3 => 2,
        TransportKind::MasqueH2 => 3,
    }
}

impl SearchSchedule {
    pub(crate) fn new(
        policy: &SelectionPolicy,
        ipv6: bool,
        extended: bool,
        start: Instant,
        seed: u64,
        parameters: &[ParameterSet],
    ) -> Result<Self> {
        let kinds: Vec<_> = [
            TransportKind::WireGuard,
            TransportKind::Awg,
            TransportKind::MasqueH3,
            TransportKind::MasqueH2,
        ]
        .into_iter()
        .filter(|kind| policy.allows_kind(*kind))
        .collect();
        let work = Duration::from_secs(if extended { 510 } else { 135 });
        let per_transport = work * 2 / (kinds.len() as u32).max(2);
        let mut variants = Vec::new();
        for &kind in &kinds {
            let mut matching = Vec::new();
            for parameter in parameters {
                let transport = parameter.runtime_transport()?;
                if transport.kind() == kind && !matching.iter().any(|(_, value)| value == &transport) {
                    matching.push((parameter.name.clone(), transport));
                }
            }
            ensure!(
                !matching.is_empty(),
                "Worker lacks an allowed transport's concrete parameters"
            );
            variants.push((kind, matching));
        }
        let mut streams = Vec::new();
        let mut initial = VecDeque::new();
        for variant in 0..variants.iter().map(|(_, values)| values.len()).max().unwrap_or(0) {
            for family in [false, true].into_iter().filter(|family| !*family || ipv6) {
                for (kind, values) in &variants {
                    let Some((name, transport)) = values.get(variant) else {
                        continue;
                    };
                    if variant == 0 {
                        initial.push_back(streams.len());
                    }
                    streams.push(Stream {
                        kind: *kind,
                        parameter_name: name.clone(),
                        transport: transport.clone(),
                        ipv6: family,
                        targets: crate::core::warp_targets::targets(*kind, family, extended, seed).into(),
                    });
                }
            }
        }
        Ok(Self {
            initial,
            expansion: (0..streams.len()).collect(),
            streams,
            active: HashMap::new(),
            spent: [Duration::ZERO; 4],
            per_transport,
            discovery_deadline: start + work,
            next_token: 0,
        })
    }

    pub(crate) fn discovery_deadline(&self) -> Instant {
        self.discovery_deadline
    }

    pub(crate) fn baseline_profiles(&self) -> Result<Vec<crate::config::warp_profiles::WarpProfile>> {
        let mut profiles = Vec::new();
        for &index in &self.initial {
            let stream = &self.streams[index];
            let count = match (stream.kind, stream.ipv6) {
                (TransportKind::MasqueH3 | TransportKind::MasqueH2, true) => 4,
                (TransportKind::MasqueH3 | TransportKind::MasqueH2, false) => 2,
                _ => 1,
            };
            for &endpoint in stream.targets.iter().take(count) {
                profiles.push(crate::config::warp_profiles::WarpProfile::new(endpoint, stream.transport.clone())?);
            }
        }
        Ok(profiles)
    }

    pub(crate) fn disable(&mut self, kind: TransportKind) {
        for stream in &mut self.streams {
            if stream.kind == kind {
                stream.targets.clear();
            }
        }
    }

    pub(crate) fn admit(&mut self, now: Instant) -> Option<ScheduledBatch> {
        if now >= self.discovery_deadline || self.active.len() >= 2 {
            return None;
        }
        let initial = !self.initial.is_empty();
        if !initial && self.active.values().any(|job| job.initial) {
            return None;
        }
        let attempts = if initial {
            self.initial.len()
        } else {
            self.expansion.len()
        };
        for _ in 0..attempts {
            let index = if initial {
                self.initial.pop_front()?
            } else {
                self.expansion.pop_front()?
            };
            let kind = self.streams[index].kind;
            let budget = self.per_transport.saturating_sub(self.spent[slot(kind)]);
            if budget.is_zero() || self.streams[index].targets.is_empty() {
                continue;
            }
            let reserved = self
                .active
                .values()
                .filter(|job| self.streams[job.stream].kind == kind)
                .map(|job| job.deadline.saturating_duration_since(job.started))
                .sum::<Duration>();
            let available = budget.saturating_sub(reserved);
            if available.is_zero() {
                if initial {
                    self.initial.push_back(index);
                } else {
                    self.expansion.push_back(index);
                }
                continue;
            }
            let deadline = self
                .discovery_deadline
                .min(now + available.min(Duration::from_secs(if initial { 10 } else { 20 })));
            let stream = &mut self.streams[index];
            let count = stream.targets.len().min(if initial { 4 } else { 8 });
            let targets = stream.targets.drain(..count).collect();
            if !initial && !stream.targets.is_empty() {
                self.expansion.push_back(index);
            }
            let token = self.next_token;
            self.next_token += 1;
            self.active.insert(
                token,
                Active {
                    stream: index,
                    initial,
                    started: now,
                    deadline,
                },
            );
            return Some(ScheduledBatch {
                token,
                kind,
                parameter_name: stream.parameter_name.clone(),
                transport: stream.transport.clone(),
                ipv6: stream.ipv6,
                initial,
                targets,
                timeout_ms: deadline.saturating_duration_since(now).as_millis().clamp(1, 8000) as u32,
                deadline,
            });
        }
        None
    }

    pub(crate) fn finish(&mut self, token: u64, now: Instant) -> bool {
        let Some(job) = self.active.remove(&token) else {
            return false;
        };
        self.spent[slot(self.streams[job.stream].kind)] += now.saturating_duration_since(job.started);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parameters() -> Vec<super::super::wire::ParameterSet> {
        let event =
            super::super::wire::decode_event(include_bytes!("../../../../protocol/fixtures/search-v1-tuning.json"))
                .unwrap();
        let super::super::wire::EventPayload::Hello(hello) = event.payload else {
            panic!("missing fixture hello")
        };
        hello.parameter_sets
    }

    #[test]
    fn every_allowed_transport_and_family_starts_before_expansion() {
        let start = Instant::now();
        let mut plan = SearchSchedule::new(&SelectionPolicy::default(), true, false, start, 17, &parameters()).unwrap();
        let first = plan.admit(start).unwrap();
        let second = plan.admit(start).unwrap();
        assert!(plan.admit(start).is_none());
        let mut started = vec![(first.kind, first.ipv6), (second.kind, second.ipv6)];
        assert!(first.initial && second.initial);
        plan.finish(first.token, start + Duration::from_secs(1));
        plan.finish(second.token, start + Duration::from_secs(1));
        for _ in 0..5 {
            let job = plan.admit(start + Duration::from_secs(1)).unwrap();
            assert!(job.initial);
            started.push((job.kind, job.ipv6));
            plan.finish(job.token, start + Duration::from_secs(2));
        }
        let last = plan.admit(start + Duration::from_secs(2)).unwrap();
        assert!(last.initial);
        started.push((last.kind, last.ipv6));
        assert!(plan.admit(start + Duration::from_secs(2)).is_none());
        plan.finish(last.token, start + Duration::from_secs(3));
        assert!(!plan.admit(start + Duration::from_secs(3)).unwrap().initial);
        for kind in [
            TransportKind::WireGuard,
            TransportKind::Awg,
            TransportKind::MasqueH3,
            TransportKind::MasqueH2,
        ] {
            assert!(started.contains(&(kind, false)));
            assert!(started.contains(&(kind, true)));
        }
    }

    #[test]
    fn h2_only_policy_never_creates_wireguard_or_other_protocol_jobs() {
        let start = Instant::now();
        let policy = SelectionPolicy {
            allowed_transports: vec![TransportKind::MasqueH2],
            ..Default::default()
        };
        let mut plan = SearchSchedule::new(&policy, false, true, start, 38, &parameters()).unwrap();
        for _ in 0..5 {
            let job = plan.admit(start).unwrap();
            assert_eq!(job.kind, TransportKind::MasqueH2);
            assert!(!job.ipv6);
            assert!(job.targets.len() <= 8);
            plan.finish(job.token, start + Duration::from_secs(1));
        }
    }

    #[test]
    fn discovery_preserves_runtime_validation_and_cleanup_reserve() {
        for (extended, work_seconds) in [(false, 135), (true, 510)] {
            let start = Instant::now();
            let mut plan =
                SearchSchedule::new(&SelectionPolicy::default(), false, extended, start, 1, &parameters()).unwrap();
            let job = plan.admit(start + Duration::from_secs(work_seconds - 1)).unwrap();
            assert!(job.deadline <= start + Duration::from_secs(work_seconds));
            assert!(plan.admit(start + Duration::from_secs(work_seconds)).is_none());
        }
    }

    #[test]
    fn tuning_batches_keep_exact_values_and_interleave_protocols_before_repeating_variants() {
        let start = Instant::now();
        let profiles = parameters();
        let mut plan = SearchSchedule::new(&SelectionPolicy::default(), false, true, start, 58, &profiles).unwrap();
        let mut now = start;
        for expected in [
            TransportKind::WireGuard,
            TransportKind::Awg,
            TransportKind::MasqueH3,
            TransportKind::MasqueH2,
        ] {
            let job = plan.admit(now).unwrap();
            assert!(job.initial);
            assert_eq!(job.kind, expected);
            now += Duration::from_millis(1);
            plan.finish(job.token, now);
        }
        let mut names = std::collections::HashSet::new();
        for expected in profiles.iter().map(|profile| &profile.name) {
            names.insert(expected.as_str());
        }
        let mut seen = std::collections::HashSet::new();
        for _ in 0..profiles.len() {
            let job = plan.admit(now).unwrap();
            let advertised = profiles
                .iter()
                .find(|profile| profile.name == job.parameter_name)
                .unwrap();
            assert_eq!(job.transport, advertised.runtime_transport().unwrap());
            assert_eq!(job.transport.kind(), job.kind);
            assert!(job.targets.iter().all(|endpoint| {
                crate::config::warp_profiles::WarpProfile::new(*endpoint, job.transport.clone()).is_ok()
            }));
            seen.insert(job.parameter_name);
            now += Duration::from_millis(1);
            plan.finish(job.token, now);
        }
        assert_eq!(seen.len(), names.len());
        assert!(seen.contains("awg-dns-google-i1"));
        assert!(seen.contains("masqueH2-google"));
        assert!(SearchSchedule::new(&SelectionPolicy::default(), false, true, start, 1, &profiles[..1]).is_err());
    }
}
