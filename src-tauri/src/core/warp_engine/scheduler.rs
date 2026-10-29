use super::wire::ParameterSet;
use crate::config::{
    warp_profiles::{TransportKind, WarpTransport},
    warp_selection::SelectionPolicy,
};
use anyhow::{Result, ensure};
use std::{
    collections::{HashMap, HashSet, VecDeque},
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
    policy: SelectionPolicy,
    ipv6: bool,
    advanced: bool,
    parameters: Vec<ParameterSet>,
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

fn is_advanced_parameter(name: &str) -> bool {
    name.contains("junk")
        || name.contains("small")
        || name.contains("wide")
        || (name.starts_with("masque") && !name.ends_with("-default"))
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
        Self::new_with_tuning(policy, ipv6, extended, extended, start, seed, parameters)
    }

    pub(crate) fn new_with_tuning(
        policy: &SelectionPolicy,
        ipv6: bool,
        extended: bool,
        advanced: bool,
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
                if !advanced && is_advanced_parameter(&parameter.name) {
                    continue;
                }
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
            policy: policy.clone(),
            ipv6,
            advanced,
            parameters: parameters.to_vec(),
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
        let mut profiles: Vec<crate::config::warp_profiles::WarpProfile> = Vec::new();
        for &index in &self.initial {
            let stream = &self.streams[index];
            let count = match (stream.kind, stream.ipv6) {
                (TransportKind::MasqueH3 | TransportKind::MasqueH2, true) => 4,
                (TransportKind::MasqueH3 | TransportKind::MasqueH2, false) => 2,
                _ => 1,
            };
            for &endpoint in stream.targets.iter().take(count) {
                let profile = crate::config::warp_profiles::WarpProfile::new(endpoint, stream.transport.clone())?;
                if !profiles.iter().any(|p| p.id == profile.id) {
                    profiles.push(profile);
                }
            }
        }
        Ok(profiles)
    }

    pub(crate) fn prepend_saved(&mut self, saved: &[crate::config::warp_profiles::WarpProfile]) -> Result<()> {
        ensure!(
            self.active.is_empty(),
            "Cannot prepend saved profiles after admission has begun"
        );
        let canonical: Vec<_> = self.initial.iter().copied().collect();
        let mut groups: Vec<VecDeque<usize>> = vec![VecDeque::new(); canonical.len()];
        let mut seen = HashSet::new();
        for profile in saved {
            if !self.policy.allows_transport(&profile.transport) {
                continue;
            }
            if profile.endpoint.is_ipv6() && !self.ipv6 {
                continue;
            }
            if !seen.insert(&profile.id) {
                continue;
            }
            let kind = profile.transport.kind();
            let Some(group) = canonical.iter().position(|&index| {
                self.streams[index].kind == kind && self.streams[index].ipv6 == profile.endpoint.is_ipv6()
            }) else {
                continue;
            };
            let parameter_name = self
                .parameters
                .iter()
                .find(|p| p.runtime_transport().map(|t| t == profile.transport).unwrap_or(false))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| format!("{}-saved", profile.transport.label()));

            let stream_idx = self.streams.len();
            self.streams.push(Stream {
                kind,
                parameter_name,
                transport: profile.transport.clone(),
                ipv6: profile.endpoint.is_ipv6(),
                targets: VecDeque::from([profile.endpoint]),
            });
            groups[group].push_back(stream_idx);
        }
        self.initial.clear();
        for group in &mut groups {
            if let Some(index) = group.pop_front() {
                self.initial.push_back(index);
            }
        }
        self.initial.extend(canonical);
        let mut saved_expansion = VecDeque::new();
        loop {
            let before = saved_expansion.len();
            for group in &mut groups {
                if let Some(index) = group.pop_front() {
                    saved_expansion.push_back(index);
                }
            }
            if saved_expansion.len() == before {
                break;
            }
        }
        saved_expansion.append(&mut self.expansion);
        self.expansion = saved_expansion;
        Ok(())
    }

    pub(crate) fn initial_completed(&self) -> bool {
        self.initial.is_empty() && !self.active.values().any(|job| job.initial)
    }

    pub(crate) fn can_resume(&self, now: Instant) -> bool {
        self.initial_completed()
            && self.active.is_empty()
            && now < self.discovery_deadline
            && self.expansion.iter().any(|&index| {
                let stream = &self.streams[index];
                !stream.targets.is_empty() && self.spent[slot(stream.kind)] < self.per_transport
            })
    }

    pub(crate) fn is_advanced(&self) -> bool {
        self.advanced
    }

    pub(crate) fn disable(&mut self, kind: TransportKind) {
        for stream in &mut self.streams {
            if stream.kind == kind {
                stream.targets.clear();
            }
        }
        self.initial.retain(|&index| self.streams[index].kind != kind);
        self.expansion.retain(|&index| self.streams[index].kind != kind);
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
    fn continuation_keeps_the_remaining_queue_and_original_deadline() -> anyhow::Result<()> {
        let start = Instant::now();
        let policy = SelectionPolicy {
            allowed_transports: vec![TransportKind::MasqueH2],
            ..Default::default()
        };
        let mut plan = SearchSchedule::new(&policy, false, false, start, 17, &parameters())?;
        let first = plan.admit(start).unwrap();
        assert!(!plan.can_resume(start));
        assert!(plan.finish(first.token, start + Duration::from_secs(10)));
        assert!(plan.can_resume(start + Duration::from_secs(30)));
        let next = plan.admit(start + Duration::from_secs(30)).unwrap();
        assert!(!next.initial);
        assert!(next.token > first.token);
        assert!(next.targets.iter().all(|target| !first.targets.contains(target)));
        assert_eq!(plan.discovery_deadline(), start + Duration::from_secs(135));
        plan.finish(next.token, next.deadline);
        let mut now = next.deadline;
        while let Some(batch) = plan.admit(now) {
            now = batch.deadline;
            plan.finish(batch.token, now);
        }
        assert!(!plan.can_resume(now));
        assert!(!plan.can_resume(start + Duration::from_secs(135)));
        Ok(())
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

    #[test]
    fn saved_h2_parameters_are_checked_without_losing_standard_targets() -> anyhow::Result<()> {
        use crate::config::warp_profiles::{WarpProfile, WarpTransport};
        let start = Instant::now();
        let policy = SelectionPolicy {
            allowed_transports: vec![TransportKind::MasqueH2],
            ..Default::default()
        };
        let saved = WarpProfile::new(
            "162.159.199.155:443".parse()?,
            WarpTransport::MasqueH2 {
                sni: "www.google.com".into(),
            },
        )?;
        let mut plan = SearchSchedule::new(&policy, false, false, start, 38, &parameters())?;
        plan.prepend_saved(&[saved.clone()])?;
        let first = plan.admit(start).unwrap();
        assert_eq!(first.transport, saved.transport);
        assert_eq!(first.targets[0], saved.endpoint);
        plan.finish(first.token, start + Duration::from_millis(1));
        let baseline = plan.admit(start + Duration::from_millis(1)).unwrap();
        assert!(baseline.targets.contains(&"162.159.198.2:443".parse()?));
        assert!(baseline.targets.contains(&"162.159.198.1:443".parse()?));
        Ok(())
    }

    #[test]
    fn slow_saved_profiles_cannot_exhaust_the_standard_endpoint_budget() -> anyhow::Result<()> {
        use crate::config::warp_profiles::WarpProfile;
        for ipv6 in [false, true] {
            let start = Instant::now();
            let policy = SelectionPolicy {
                allowed_transports: vec![TransportKind::MasqueH2],
                ..Default::default()
            };
            let transport = WarpTransport::MasqueH2 {
                sni: "www.google.com".into(),
            };
            let mut saved = Vec::new();
            for family in [false, true].into_iter().filter(|family| !*family || ipv6) {
                for host in 10..50 {
                    let endpoint = if family {
                        format!("[2606:4700:103::{host:x}]:443")
                    } else {
                        format!("162.159.199.{host}:443")
                    };
                    saved.push(WarpProfile::new(endpoint.parse()?, transport.clone())?);
                }
            }
            let mut plan = SearchSchedule::new(&policy, ipv6, false, start, 38, &parameters())?;
            plan.prepend_saved(&saved)?;
            let mut now = start;
            let mut standard_v4 = false;
            let mut standard_v6 = false;
            while now < start + Duration::from_secs(60) {
                let Some(batch) = plan.admit(now) else { break };
                standard_v4 |= batch.targets.contains(&"162.159.198.2:443".parse()?);
                standard_v6 |= batch.targets.contains(&"[2606:4700:103::2]:443".parse()?);
                now = batch.deadline;
                plan.finish(batch.token, now);
                if standard_v4 && (!ipv6 || standard_v6) {
                    break;
                }
            }
            assert!(standard_v4, "saved profiles displaced the IPv4 standard endpoint");
            assert!(
                !ipv6 || standard_v6,
                "saved profiles displaced the IPv6 standard endpoint"
            );
        }
        Ok(())
    }

    #[test]
    fn advanced_mode_controls_wide_junk_and_alternate_sni_admission() -> anyhow::Result<()> {
        use crate::config::warp_profiles::{WarpProfile, WarpTransport};
        let start = Instant::now();
        let params = parameters();

        let mut normal =
            SearchSchedule::new_with_tuning(&SelectionPolicy::default(), false, true, false, start, 123, &params)?;
        assert!(!normal.is_advanced());
        let mut normal_names = Vec::new();
        let mut now = start;
        while let Some(batch) = normal.admit(now) {
            normal_names.push(batch.parameter_name.clone());
            now += Duration::from_millis(1);
            normal.finish(batch.token, now);
        }
        assert!(normal_names.iter().any(|name| name == "awg-default"));
        assert!(normal_names.iter().any(|name| name == "awg-dns-google-i1"));
        assert!(normal_names.iter().any(|name| name == "masqueH2-default"));
        assert!(!normal_names.iter().any(|name| name == "awg-junk-wide"));
        assert!(!normal_names.iter().any(|name| name == "awg-dns-google-wide"));
        assert!(!normal_names.iter().any(|name| name == "masqueH2-google"));
        assert!(!normal_names.iter().any(|name| name == "masqueH3-apple"));

        let mut advanced =
            SearchSchedule::new_with_tuning(&SelectionPolicy::default(), false, true, true, start, 123, &params)?;
        assert!(advanced.is_advanced());
        let mut advanced_names = Vec::new();
        now = start;
        while let Some(batch) = advanced.admit(now) {
            advanced_names.push(batch.parameter_name.clone());
            now += Duration::from_millis(1);
            advanced.finish(batch.token, now);
        }
        assert!(advanced_names.iter().any(|name| name == "awg-junk-wide"));
        assert!(advanced_names.iter().any(|name| name == "awg-dns-google-wide"));
        assert!(advanced_names.iter().any(|name| name == "masqueH2-google"));
        assert!(advanced_names.iter().any(|name| name == "masqueH3-apple"));

        let saved = WarpProfile::new(
            "162.159.199.155:443".parse()?,
            WarpTransport::MasqueH2 {
                sni: "www.google.com".into(),
            },
        )?;
        let mut manual_plan = SearchSchedule::new_with_tuning(
            &SelectionPolicy {
                allowed_transports: vec![TransportKind::MasqueH2],
                ..Default::default()
            },
            false,
            false,
            false,
            start,
            123,
            &params,
        )?;
        manual_plan.prepend_saved(&[saved.clone()])?;
        let manual_batch = manual_plan.admit(start).unwrap();
        assert_eq!(manual_batch.transport, saved.transport);
        assert_eq!(manual_batch.targets[0], saved.endpoint);

        Ok(())
    }

    #[test]
    fn early_handoff_requires_every_initial_transport_including_late_h2() -> anyhow::Result<()> {
        let start = Instant::now();
        let params = parameters();
        let mut plan = SearchSchedule::new(&SelectionPolicy::default(), false, false, start, 99, &params)?;

        let b1 = plan.admit(start).unwrap();
        let b2 = plan.admit(start).unwrap();
        assert!(b1.initial && b2.initial);
        assert!(!plan.initial_completed());

        let now = start + Duration::from_millis(10);
        plan.finish(b1.token, now);
        assert!(!plan.initial_completed());
        let b3 = plan.admit(now).unwrap();
        assert!(b3.initial);

        let now = now + Duration::from_millis(10);
        plan.finish(b2.token, now);
        assert!(!plan.initial_completed());
        let b4 = plan.admit(now).unwrap();
        assert!(b4.initial);

        let now = now + Duration::from_millis(10);
        plan.finish(b3.token, now);
        assert!(!plan.initial_completed());

        let now = now + Duration::from_millis(10);
        plan.finish(b4.token, now);
        assert!(plan.initial_completed());

        Ok(())
    }

    #[test]
    fn early_handoff_handles_disabled_transport_from_typed_failure() -> anyhow::Result<()> {
        let start = Instant::now();
        let params = parameters();
        let mut plan = SearchSchedule::new(&SelectionPolicy::default(), false, false, start, 99, &params)?;

        plan.disable(TransportKind::WireGuard);
        plan.disable(TransportKind::Awg);
        assert!(!plan.initial_completed());

        let b1 = plan.admit(start).unwrap();
        let b2 = plan.admit(start).unwrap();
        assert!(b1.kind == TransportKind::MasqueH3 || b1.kind == TransportKind::MasqueH2);
        assert!(b2.kind == TransportKind::MasqueH3 || b2.kind == TransportKind::MasqueH2);
        assert!(!plan.initial_completed());

        let now = start + Duration::from_millis(10);
        plan.finish(b1.token, now);
        assert!(!plan.initial_completed());

        let now = now + Duration::from_millis(10);
        plan.finish(b2.token, now);
        assert!(plan.initial_completed());

        Ok(())
    }
}
