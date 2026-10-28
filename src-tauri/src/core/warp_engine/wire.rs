use anyhow::{Result, anyhow, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::{HashMap, HashSet},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub(crate) const VERSION: u32 = 1;
pub(crate) const MAX_FRAME_BYTES: usize = 262144;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum Transport {
    #[serde(rename = "wg")]
    WireGuard,
    #[serde(rename = "awg")]
    Awg,
    #[serde(rename = "masqueH3")]
    MasqueH3,
    #[serde(rename = "masqueH2")]
    MasqueH2,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AwgParameters {
    pub(crate) jc: u16,
    pub(crate) jmin: u16,
    pub(crate) jmax: u16,
    pub(crate) i1: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Parameters {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) awg: Option<AwgParameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sni: Option<String>,
}

impl Parameters {
    fn matches(&self, transport: Transport) -> bool {
        match transport {
            Transport::WireGuard => self.awg.is_none() && self.sni.is_none(),
            Transport::Awg => self.awg.is_some() && self.sni.is_none(),
            Transport::MasqueH3 | Transport::MasqueH2 => {
                self.awg.is_none() && self.sni.as_ref().is_some_and(|sni| !sni.is_empty())
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WgIdentity {
    pub(crate) private_key: String,
    pub(crate) peer_public_key: String,
    pub(crate) ipv4: Ipv4Addr,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ipv6: Option<Ipv6Addr>,
    pub(crate) reserved: [u8; 3],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EcIdentity {
    pub(crate) private_key_der: String,
    pub(crate) peer_public_key_der: String,
    pub(crate) ipv4: Ipv4Addr,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ipv6: Option<Ipv6Addr>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Identity {
    WireGuard(WgIdentity),
    Masque(EcIdentity),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeRequest {
    pub(crate) transport: Transport,
    pub(crate) ipv6: bool,
    pub(crate) targets: Vec<SocketAddr>,
    pub(crate) identity: Identity,
    pub(crate) parameters: Parameters,
    pub(crate) timeout_ms: u32,
    pub(crate) tunnel_limit: u8,
}

impl ProbeRequest {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            (1..=30000).contains(&self.timeout_ms)
                && (1..=4).contains(&self.tunnel_limit)
                && (1..=256).contains(&self.targets.len()),
            "Invalid worker request limits"
        );
        ensure!(
            self.targets
                .iter()
                .all(|target| target.port() > 0 && target.is_ipv6() == self.ipv6),
            "Invalid worker endpoint family or port"
        );
        ensure!(
            self.parameters.matches(self.transport),
            "Invalid worker transport parameters"
        );
        let identity_matches = match (&self.identity, self.transport) {
            (Identity::WireGuard(identity), Transport::WireGuard | Transport::Awg) => {
                !identity.private_key.is_empty() && !identity.peer_public_key.is_empty()
            }
            (Identity::Masque(identity), Transport::MasqueH3 | Transport::MasqueH2) => {
                !identity.private_key_der.is_empty() && !identity.peer_public_key_der.is_empty()
            }
            _ => false,
        };
        ensure!(identity_matches, "Invalid worker identity family");
        Ok(())
    }
}

fn required_nullable<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Stages {
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) handshake: Option<bool>,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) http: Option<bool>,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) https: Option<bool>,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) udp: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Candidate {
    pub(crate) endpoint: SocketAddr,
    pub(crate) transport: Transport,
    pub(crate) parameters: Parameters,
    pub(crate) stages: Stages,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) endpoint_ping_ms: Option<f64>,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) tunnel_ping_ms: Option<f64>,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) loss_percent: Option<f64>,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) jitter_ms: Option<f64>,
    pub(crate) samples: u32,
    pub(crate) method: String,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) node: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) node_country: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) exit_country: Option<String>,
}

impl Candidate {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.endpoint.port() > 0 && self.parameters.matches(self.transport),
            "Invalid worker candidate transport"
        );
        ensure!(
            [
                self.endpoint_ping_ms,
                self.tunnel_ping_ms,
                self.loss_percent,
                self.jitter_ms
            ]
            .into_iter()
            .flatten()
            .all(|value| value.is_finite() && value >= 0.0),
            "Invalid worker measurements"
        );
        ensure!(
            self.loss_percent.is_none_or(|value| value <= 100.0),
            "Invalid worker loss"
        );
        if self.tunnel_ping_ms.is_some() || self.loss_percent.is_some() || self.jitter_ms.is_some() {
            ensure!(
                self.samples > 0 && !self.method.is_empty(),
                "Missing worker measurement provenance"
            );
        }
        ensure!(
            [&self.node_country, &self.exit_country]
                .into_iter()
                .flatten()
                .all(|country| country.len() == 2 && country.bytes().all(|byte| byte.is_ascii_uppercase())),
            "Invalid worker country"
        );
        ensure!(
            self.node.as_ref().is_none_or(|node| !node.is_empty()),
            "Invalid worker node"
        );
        if self.stages.https == Some(true) || self.stages.udp == Some(true) {
            ensure!(self.stages.handshake == Some(true), "Worker data without handshake");
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Hello {
    pub(crate) build_id: String,
    pub(crate) transports: Vec<Transport>,
    pub(crate) tuning: Vec<String>,
    #[serde(default)]
    pub(crate) parameter_sets: Vec<ParameterSet>,
    #[serde(default)]
    pub(crate) unavailable_tuning: Vec<UnavailableTuning>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParameterSet {
    pub(crate) name: String,
    pub(crate) transport: Transport,
    pub(crate) parameters: Parameters,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnavailableTuning {
    pub(crate) name: String,
    pub(crate) reason: String,
}

impl ParameterSet {
    pub(crate) fn runtime_transport(&self) -> Result<crate::config::warp_profiles::WarpTransport> {
        use crate::config::warp_profiles::{WarpProfile, WarpTransport};
        ensure!(
            self.parameters.matches(self.transport),
            "Invalid worker tuning parameters"
        );
        let transport = match self.transport {
            Transport::WireGuard => WarpTransport::WireGuard,
            Transport::Awg => {
                let parameters = self
                    .parameters
                    .awg
                    .as_ref()
                    .ok_or_else(|| anyhow!("Missing AWG tuning parameters"))?;
                WarpTransport::Awg {
                    jc: parameters.jc,
                    jmin: parameters.jmin,
                    jmax: parameters.jmax,
                    i1: parameters.i1.clone(),
                }
            }
            Transport::MasqueH3 => WarpTransport::MasqueH3 {
                sni: self
                    .parameters
                    .sni
                    .clone()
                    .ok_or_else(|| anyhow!("Missing MASQUE tuning parameters"))?,
            },
            Transport::MasqueH2 => WarpTransport::MasqueH2 {
                sni: self
                    .parameters
                    .sni
                    .clone()
                    .ok_or_else(|| anyhow!("Missing MASQUE tuning parameters"))?,
            },
        };
        let endpoint = match self.transport {
            Transport::WireGuard | Transport::Awg => "188.114.98.58:2408",
            Transport::MasqueH3 | Transport::MasqueH2 => "162.159.198.2:443",
        };
        WarpProfile::new(endpoint.parse()?, transport.clone())?;
        Ok(transport)
    }
}

impl Hello {
    fn validate(&self) -> Result<()> {
        let valid_name = |name: &str| {
            !name.is_empty()
                && name.len() <= 64
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        };
        ensure!(
            self.parameter_sets.len() <= 32 && self.unavailable_tuning.len() <= 32 && self.tuning.len() <= 64,
            "Invalid worker tuning catalog limits"
        );
        let mut names = HashSet::new();
        for profile in &self.parameter_sets {
            ensure!(
                valid_name(&profile.name)
                    && names.insert(&profile.name)
                    && self.transports.contains(&profile.transport)
                    && self.tuning.contains(&profile.name),
                "Invalid worker tuning capability"
            );
            profile.runtime_transport()?;
        }
        for unavailable in &self.unavailable_tuning {
            ensure!(
                valid_name(&unavailable.name) && valid_name(&unavailable.reason) && names.insert(&unavailable.name),
                "Invalid unavailable tuning capability"
            );
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Progress {
    pub(crate) completed: u32,
    pub(crate) total: u32,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Outcome {
    Completed,
    Cancelled,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Finished {
    outcome: Outcome,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Failure {
    pub(crate) stage: String,
    pub(crate) reason: String,
    pub(crate) retryable: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
enum CommandPayload<'a> {
    Hello(Empty),
    Probe(&'a ProbeRequest),
    Cancel(Empty),
    Shutdown(Empty),
}

pub(crate) enum Command<'a> {
    Hello,
    Probe(&'a ProbeRequest),
    Cancel,
    Shutdown,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandEnvelope<'a> {
    version: u32,
    search_id: &'a str,
    job_id: &'a str,
    #[serde(flatten)]
    command: CommandPayload<'a>,
}

pub(crate) async fn write_command<W: AsyncWrite + Unpin>(
    writer: &mut W,
    search_id: &str,
    job_id: &str,
    command: Command<'_>,
) -> Result<()> {
    ensure!(
        valid_id(search_id) && valid_id(job_id),
        "Invalid worker command identifiers"
    );
    let command = match command {
        Command::Hello => CommandPayload::Hello(Empty {}),
        Command::Probe(request) => {
            request.validate()?;
            CommandPayload::Probe(request)
        }
        Command::Cancel => CommandPayload::Cancel(Empty {}),
        Command::Shutdown => CommandPayload::Shutdown(Empty {}),
    };
    let data = serde_json::to_vec(&CommandEnvelope {
        version: VERSION,
        search_id,
        job_id,
        command,
    })
    .map_err(|_| anyhow!("Invalid worker command"))?;
    ensure!((1..=MAX_FRAME_BYTES).contains(&data.len()), "Invalid worker frame size");
    writer
        .write_all(&(data.len() as u32).to_be_bytes())
        .await
        .map_err(|_| anyhow!("Worker input closed"))?;
    writer
        .write_all(&data)
        .await
        .map_err(|_| anyhow!("Worker input closed"))?;
    writer.flush().await.map_err(|_| anyhow!("Worker input closed"))?;
    Ok(())
}

pub(crate) enum EventPayload {
    Hello(Hello),
    Accepted,
    Progress(Progress),
    Candidate(Candidate),
    JobFinished(Outcome),
    Error(Failure),
}

pub(crate) struct EngineEvent {
    pub(crate) search_id: String,
    pub(crate) job_id: String,
    pub(crate) sequence: u64,
    pub(crate) payload: EventPayload,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawEvent {
    version: u32,
    search_id: String,
    job_id: String,
    sequence: u64,
    #[serde(rename = "type")]
    kind: String,
    payload: serde_json::Value,
}

fn payload<T: DeserializeOwned>(value: serde_json::Value) -> Result<T> {
    serde_json::from_value(value).map_err(|_| anyhow!("Invalid worker event payload"))
}

pub(crate) fn decode_event(data: &[u8]) -> Result<EngineEvent> {
    ensure!(
        !data.is_empty() && data.len() <= MAX_FRAME_BYTES,
        "Invalid worker frame size"
    );
    let event: RawEvent = serde_json::from_slice(data).map_err(|_| anyhow!("Invalid worker event"))?;
    ensure!(
        event.version == VERSION && valid_id(&event.search_id) && valid_id(&event.job_id) && event.sequence > 0,
        "Invalid worker event envelope"
    );
    let decoded = match event.kind.as_str() {
        "hello" => {
            let hello: Hello = payload(event.payload)?;
            hello.validate()?;
            EventPayload::Hello(hello)
        }
        "accepted" => {
            let _: Empty = payload(event.payload)?;
            EventPayload::Accepted
        }
        "progress" => EventPayload::Progress(payload(event.payload)?),
        "candidate" => {
            let candidate: Candidate = payload(event.payload)?;
            candidate.validate()?;
            EventPayload::Candidate(candidate)
        }
        "jobFinished" => EventPayload::JobFinished(payload::<Finished>(event.payload)?.outcome),
        "error" => {
            let failure: Failure = payload(event.payload)?;
            ensure!(
                !failure.stage.is_empty() && !failure.reason.is_empty(),
                "Invalid worker failure"
            );
            EventPayload::Error(failure)
        }
        _ => return Err(anyhow!("Unknown worker event type")),
    };
    Ok(EngineEvent {
        search_id: event.search_id,
        job_id: event.job_id,
        sequence: event.sequence,
        payload: decoded,
    })
}

pub(crate) async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    let mut header = [0; 4];
    reader
        .read_exact(&mut header)
        .await
        .map_err(|_| anyhow!("Worker output ended"))?;
    let size = u32::from_be_bytes(header) as usize;
    ensure!((1..=MAX_FRAME_BYTES).contains(&size), "Invalid worker frame size");
    let mut data = vec![0; size];
    reader
        .read_exact(&mut data)
        .await
        .map_err(|_| anyhow!("Truncated worker frame"))?;
    Ok(data)
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JobPhase {
    Submitted,
    Running,
    Terminal,
}

struct TrackedJob {
    phase: JobPhase,
    sequence: u64,
    transport: Transport,
    parameters: Parameters,
    targets: HashSet<SocketAddr>,
    total: u32,
    completed: u32,
}

pub(crate) struct EventTracker {
    search_id: String,
    control_job: String,
    expected_build: String,
    capabilities: Option<Hello>,
    jobs: HashMap<String, TrackedJob>,
}

impl EventTracker {
    pub(crate) fn build_id(&self) -> &str {
        &self.expected_build
    }

    pub(crate) fn parameter_sets(&self) -> &[ParameterSet] {
        self.capabilities
            .as_ref()
            .map(|hello| hello.parameter_sets.as_slice())
            .unwrap_or_default()
    }

    pub(crate) fn unavailable_tuning(&self) -> &[UnavailableTuning] {
        self.capabilities
            .as_ref()
            .map(|hello| hello.unavailable_tuning.as_slice())
            .unwrap_or_default()
    }

    pub(crate) fn new(search_id: String, control_job: String, expected_build: String) -> Self {
        Self {
            search_id,
            control_job,
            expected_build,
            capabilities: None,
            jobs: HashMap::new(),
        }
    }

    pub(crate) fn register_job(&mut self, id: &str, request: &ProbeRequest) -> Result<()> {
        request.validate()?;
        let hello = self
            .capabilities
            .as_ref()
            .ok_or_else(|| anyhow!("Worker handshake is incomplete"))?;
        ensure!(
            valid_id(id) && id != self.control_job && !self.jobs.contains_key(id),
            "Invalid or reused worker job ID"
        );
        ensure!(
            hello.transports.contains(&request.transport),
            "Worker transport is unavailable"
        );
        ensure!(
            self.jobs.values().filter(|job| job.phase != JobPhase::Terminal).count() < 2,
            "Worker job limit reached"
        );
        self.jobs.insert(
            id.into(),
            TrackedJob {
                phase: JobPhase::Submitted,
                sequence: 0,
                transport: request.transport,
                parameters: request.parameters.clone(),
                targets: request.targets.iter().copied().collect(),
                total: request.targets.len() as u32,
                completed: 0,
            },
        );
        Ok(())
    }

    pub(crate) fn accept(&mut self, event: &EngineEvent) -> Result<()> {
        ensure!(event.search_id == self.search_id, "Foreign worker search event");
        if let EventPayload::Hello(hello) = &event.payload {
            ensure!(
                self.capabilities.is_none()
                    && event.job_id == self.control_job
                    && event.sequence == 1
                    && hello.build_id == self.expected_build,
                "Invalid worker handshake"
            );
            ensure!(
                hello.transports.iter().copied().collect::<HashSet<_>>().len() == hello.transports.len()
                    && hello.tuning.iter().collect::<HashSet<_>>().len() == hello.tuning.len(),
                "Duplicate worker capabilities"
            );
            self.capabilities = Some(hello.clone());
            return Ok(());
        }
        ensure!(self.capabilities.is_some(), "Worker handshake is incomplete");
        let job = self
            .jobs
            .get_mut(&event.job_id)
            .ok_or_else(|| anyhow!("Unknown worker job"))?;
        ensure!(
            job.phase != JobPhase::Terminal && event.sequence == job.sequence + 1,
            "Invalid worker event sequence"
        );
        let mut phase = job.phase;
        let mut completed = job.completed;
        match &event.payload {
            EventPayload::Accepted => {
                ensure!(phase == JobPhase::Submitted, "Duplicate worker acceptance");
                phase = JobPhase::Running;
            }
            EventPayload::Candidate(candidate) => {
                ensure!(
                    phase == JobPhase::Running
                        && candidate.transport == job.transport
                        && candidate.parameters == job.parameters
                        && job.targets.contains(&candidate.endpoint),
                    "Worker candidate does not match its job"
                );
            }
            EventPayload::Progress(progress) => {
                ensure!(
                    phase == JobPhase::Running
                        && progress.total == job.total
                        && progress.completed >= completed
                        && progress.completed <= job.total,
                    "Invalid worker progress"
                );
                completed = progress.completed;
            }
            EventPayload::JobFinished(outcome) => {
                ensure!(
                    phase == JobPhase::Running && (*outcome != Outcome::Completed || completed == job.total),
                    "Premature worker completion"
                );
                phase = JobPhase::Terminal;
            }
            EventPayload::Error(_) => phase = JobPhase::Terminal,
            EventPayload::Hello(_) => unreachable!(),
        }
        job.sequence = event.sequence;
        job.phase = phase;
        job.completed = completed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn commands_keep_identities_inside_framed_payloads() {
        let mut output = Vec::new();
        let request = request();
        for command in [
            Command::Hello,
            Command::Probe(&request),
            Command::Cancel,
            Command::Shutdown,
        ] {
            write_command(&mut output, "search", "one", command).await.unwrap();
        }
        let mut input = output.as_slice();
        for kind in ["hello", "probe", "cancel", "shutdown"] {
            let value: serde_json::Value = serde_json::from_slice(&read_frame(&mut input).await.unwrap()).unwrap();
            assert_eq!(value["version"], 1);
            assert_eq!(value["searchId"], "search");
            assert_eq!(value["jobId"], "one");
            assert_eq!(value["type"], kind);
            if kind == "probe" {
                assert_eq!(
                    value["payload"]["identity"]["masque"]["privateKeyDer"],
                    "synthetic-private"
                );
                assert_eq!(value["payload"]["targets"][0], "162.159.198.2:443");
                assert!(value["payload"]["identity"]["masque"].get("ipv6").is_none());
            } else {
                assert_eq!(value["payload"], serde_json::json!({}));
            }
        }
        assert!(input.is_empty());
    }

    #[tokio::test]
    async fn invalid_commands_do_not_write_partial_frames_or_expose_keys() {
        let mut output = Vec::new();
        let mut request = request();
        request.identity = Identity::Masque(EcIdentity {
            private_key_der: "sensitive-marker".repeat(MAX_FRAME_BYTES),
            peer_public_key_der: "peer".into(),
            ipv4: "172.16.0.2".parse().unwrap(),
            ipv6: None,
        });
        let error = write_command(&mut output, "search", "one", Command::Probe(&request))
            .await
            .err()
            .unwrap();
        assert!(!error.to_string().contains("sensitive-marker"));
        assert!(output.is_empty());
        assert!(write_command(&mut output, "", "one", Command::Hello).await.is_err());
        assert!(output.is_empty());
    }

    fn fixtures() -> serde_json::Value {
        serde_json::from_str(include_str!("../../../../protocol/fixtures/search-v1.json")).unwrap()
    }

    #[tokio::test]
    async fn shared_go_frames_decode_without_inventing_metrics() {
        for entry in fixtures()["cases"].as_array().unwrap() {
            let hex = entry["frameHex"].as_str().unwrap();
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            let payload = read_frame(&mut bytes.as_slice()).await.unwrap();
            assert_eq!(payload, entry["json"].as_str().unwrap().as_bytes());
            if entry["kind"] == "event" {
                let event = decode_event(&payload).unwrap();
                if let EventPayload::Candidate(candidate) = event.payload {
                    assert!(candidate.tunnel_ping_ms.is_none());
                    assert!(candidate.loss_percent.is_none());
                    assert!(candidate.stages.udp.is_none());
                }
            }
        }
    }

    #[tokio::test]
    async fn oversized_header_is_rejected_before_waiting_for_payload() {
        let header = (262145_u32).to_be_bytes();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            read_frame(&mut header.as_slice()),
        )
        .await;
        assert!(result.unwrap().is_err());
    }

    fn event(kind: &str, job: &str, sequence: u64, payload: serde_json::Value) -> EngineEvent {
        decode_event(
            &serde_json::to_vec(&serde_json::json!({
                "version":1,"searchId":"search","jobId":job,"sequence":sequence,"type":kind,"payload":payload
            }))
            .unwrap(),
        )
        .unwrap()
    }

    fn request() -> ProbeRequest {
        ProbeRequest {
            transport: Transport::MasqueH2,
            ipv6: false,
            targets: vec!["162.159.198.2:443".parse().unwrap()],
            identity: Identity::Masque(EcIdentity {
                private_key_der: "synthetic-private".into(),
                peer_public_key_der: "synthetic-peer".into(),
                ipv4: "172.16.0.2".parse().unwrap(),
                ipv6: None,
            }),
            parameters: Parameters {
                awg: None,
                sni: Some("consumer-masque.cloudflareclient.com".into()),
            },
            timeout_ms: 1000,
            tunnel_limit: 4,
        }
    }

    fn tracker() -> EventTracker {
        let mut tracker = EventTracker::new("search".into(), "control".into(), "build".into());
        tracker
            .accept(&event(
                "hello",
                "control",
                1,
                serde_json::json!({"buildId":"build","transports":["masqueH2"],"tuning":[]}),
            ))
            .unwrap();
        tracker.register_job("one", &request()).unwrap();
        tracker
            .accept(&event("accepted", "one", 1, serde_json::json!({})))
            .unwrap();
        tracker
    }

    #[test]
    fn tuning_handshake_preserves_concrete_parameters_and_rejects_invalid_catalogs() {
        let actual = decode_event(include_bytes!("../../../../protocol/fixtures/search-v1-tuning.json")).unwrap();
        let mut actual_tracker =
            EventTracker::new("tuning-validation".into(), "control".into(), "fixture-tuning".into());
        actual_tracker.accept(&actual).unwrap();
        for profile in actual_tracker.parameter_sets() {
            profile.runtime_transport().unwrap();
        }
        for kind in [
            Transport::WireGuard,
            Transport::Awg,
            Transport::MasqueH3,
            Transport::MasqueH2,
        ] {
            assert!(
                actual_tracker
                    .parameter_sets()
                    .iter()
                    .any(|profile| profile.transport == kind)
            );
        }
        let value = serde_json::json!({
            "version":1,"searchId":"search","jobId":"control","sequence":1,"type":"hello",
            "payload":{"buildId":"build","transports":["awg","masqueH2"],"tuning":["awg-dns","h2-google"],
                "parameterSets":[
                    {"name":"awg-dns","transport":"awg","parameters":{"awg":{"jc":0,"jmin":0,"jmax":0,"i1":"<r 32>"}}},
                    {"name":"h2-google","transport":"masqueH2","parameters":{"sni":"www.google.com"}}
                ],"unavailableTuning":[{"name":"awg-quic","reason":"provenance-unresolved"}]}
        });
        let decoded = decode_event(&serde_json::to_vec(&value).unwrap()).unwrap();
        let mut tracker = EventTracker::new("search".into(), "control".into(), "build".into());
        tracker.accept(&decoded).unwrap();
        let catalog = tracker.parameter_sets();
        assert_eq!(catalog.len(), 2);
        assert!(
            matches!(catalog[0].runtime_transport().unwrap(), crate::config::warp_profiles::WarpTransport::Awg { jc: 0, i1, .. } if i1 == "<r 32>")
        );
        assert!(
            matches!(catalog[1].runtime_transport().unwrap(), crate::config::warp_profiles::WarpTransport::MasqueH2 { sni } if sni == "www.google.com")
        );
        for path in [
            "duplicate",
            "invalidChain",
            "unadvertised",
            "tooMany",
            "parameterFamily",
            "unavailableCollision",
        ] {
            let mut bad = value.clone();
            match path {
                "duplicate" => bad["payload"]["parameterSets"][1]["name"] = "awg-dns".into(),
                "invalidChain" => {
                    bad["payload"]["parameterSets"][0]["parameters"]["awg"]["i1"] = "bad\ni1=secret".into()
                }
                "unadvertised" => bad["payload"]["transports"] = serde_json::json!(["masqueH2"]),
                "tooMany" => {
                    bad["payload"]["parameterSets"] =
                        serde_json::json!(vec![bad["payload"]["parameterSets"][0].clone(); 33])
                }
                "parameterFamily" => bad["payload"]["parameterSets"][0]["parameters"]["sni"] = "www.google.com".into(),
                _ => bad["payload"]["unavailableTuning"][0]["name"] = "awg-dns".into(),
            }
            assert!(decode_event(&serde_json::to_vec(&bad).unwrap()).is_err(), "{path}");
        }
    }

    #[test]
    fn foreign_replayed_and_skipped_events_do_not_advance_state() {
        let mut tracker = tracker();
        let mut foreign = event("progress", "one", 2, serde_json::json!({"completed":0,"total":1}));
        foreign.search_id = "foreign".into();
        assert!(tracker.accept(&foreign).is_err());
        assert!(
            tracker
                .accept(&event(
                    "progress",
                    "one",
                    3,
                    serde_json::json!({"completed":0,"total":1})
                ))
                .is_err()
        );
        let progress = event("progress", "one", 2, serde_json::json!({"completed":0,"total":1}));
        tracker.accept(&progress).unwrap();
        assert!(tracker.accept(&progress).is_err());
    }

    #[test]
    fn substituted_candidate_and_post_terminal_events_are_rejected() {
        let mut tracker = tracker();
        let mut payload: serde_json::Value =
            serde_json::from_str(fixtures()["cases"][1]["json"].as_str().unwrap()).unwrap();
        payload["payload"]["endpoint"] = "162.159.198.1:443".into();
        assert!(
            tracker
                .accept(&event("candidate", "one", 2, payload["payload"].clone()))
                .is_err()
        );
        payload["payload"]["endpoint"] = "162.159.198.2:443".into();
        tracker
            .accept(&event("candidate", "one", 2, payload["payload"].clone()))
            .unwrap();
        tracker
            .accept(&event(
                "jobFinished",
                "one",
                3,
                serde_json::json!({"outcome":"cancelled"}),
            ))
            .unwrap();
        assert!(
            tracker
                .accept(&event(
                    "progress",
                    "one",
                    4,
                    serde_json::json!({"completed":1,"total":1})
                ))
                .is_err()
        );
    }

    #[test]
    fn event_decode_rejects_secret_fields_and_missing_nullable_stages() {
        let mut candidate: serde_json::Value =
            serde_json::from_str(fixtures()["cases"][1]["json"].as_str().unwrap()).unwrap();
        candidate["payload"]["privateKey"] = "sensitive-marker".into();
        let error = decode_event(&serde_json::to_vec(&candidate).unwrap()).err().unwrap();
        assert!(!error.to_string().contains("sensitive-marker"));
        candidate["payload"].as_object_mut().unwrap().remove("privateKey");
        candidate["payload"]["stages"].as_object_mut().unwrap().remove("udp");
        assert!(decode_event(&serde_json::to_vec(&candidate).unwrap()).is_err());
    }
}
