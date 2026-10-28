use crate::config::warp_profiles::TransportKind;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FailureStage {
    Registration,
    Scanner,
    Validation,
    Runtime,
    Routing,
    Cleanup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FailureReason {
    Unreachable,
    NoCandidates,
    Timeout,
    InvalidOutput,
    Unsupported,
    PermissionDenied,
    ProcessExited,
    Cancelled,
    CleanupPending,
    PolicyRestricted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WarpFailure {
    pub(crate) stage: FailureStage,
    pub(crate) reason: FailureReason,
    pub(crate) retryable: bool,
    pub(crate) transport: Option<TransportKind>,
}

impl WarpFailure {
    pub(crate) fn new(stage: FailureStage, reason: FailureReason) -> Self {
        Self {
            stage,
            reason,
            retryable: matches!(
                reason,
                FailureReason::Unreachable
                    | FailureReason::NoCandidates
                    | FailureReason::Timeout
                    | FailureReason::ProcessExited
                    | FailureReason::CleanupPending
            ),
            transport: None,
        }
    }

    pub(crate) fn from_error(error: &anyhow::Error, stage: FailureStage, fallback: FailureReason) -> Self {
        if let Some(failure) = error.downcast_ref::<Self>() {
            return *failure;
        }
        let reason = if let Some(error) = error.downcast_ref::<std::io::Error>() {
            match error.kind() {
                std::io::ErrorKind::PermissionDenied => FailureReason::PermissionDenied,
                std::io::ErrorKind::TimedOut => FailureReason::Timeout,
                std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::NetworkUnreachable
                | std::io::ErrorKind::HostUnreachable => FailureReason::Unreachable,
                _ => fallback,
            }
        } else if let Some(error) = error.downcast_ref::<reqwest::Error>() {
            if error.is_timeout() {
                FailureReason::Timeout
            } else if error.is_connect() {
                FailureReason::Unreachable
            } else {
                fallback
            }
        } else {
            fallback
        };
        Self::new(stage, reason)
    }

    pub(crate) fn for_transport(mut self, transport: TransportKind) -> Self {
        self.transport = Some(transport);
        self
    }

    pub(crate) fn message(&self) -> &'static str {
        use FailureReason::*;
        match (self.stage, self.reason) {
            (_, PolicyRestricted) => {
                "Сервер не соответствует ограничениям подключения или его узел не подтверждён. Повторите поиск, измените ограничения или выберите другой сервер."
            }
            (_, Unsupported) => "Этот режим подключения пока не поддерживается. Выберите другой сервер.",
            (_, CleanupPending) => "Не удалось завершить подключение. Повторите отключение перед выходом.",
            (_, Cancelled) => "Операция отменена.",
            (_, PermissionDenied) => "Windows отказала в доступе. Проверьте права запуска и доступ к папке Tropa.",
            (FailureStage::Registration, Unreachable | Timeout) => {
                "Не удалось связаться с Cloudflare для регистрации. Проверьте Интернет и повторите попытку."
            }
            (FailureStage::Registration, _) => {
                "Не удалось подготовить регистрацию WARP. Проверьте доступ к папке Tropa; сохранённые реквизиты не сброшены."
            }
            (FailureStage::Scanner, NoCandidates) => {
                "Поиск завершён: рабочих адресов не найдено. Повторите поиск позже."
            }
            (FailureStage::Scanner, Timeout) => {
                "Время поиска истекло. Повторите поиск; сохранённые серверы не удалены."
            }
            (FailureStage::Scanner, ProcessExited) => {
                "WarpScout завершился с ошибкой. Проверьте комплект файлов Tropa и повторите поиск."
            }
            (FailureStage::Scanner, _) => "Некорректный результат или файлы WarpScout. Проверьте комплект Tropa.",
            (FailureStage::Runtime, ProcessExited) => "Ядро подключения остановилось. Повторите подключение.",
            (FailureStage::Runtime, _) => "Не удалось запустить ядро подключения. Проверьте комплект файлов Tropa.",
            (FailureStage::Routing, _) => {
                "Не удалось применить маршрутизацию. Проверьте выбранные EXE и повторите подключение."
            }
            (_, Timeout) => {
                "Проверка подключения превысила время ожидания. Повторите попытку или выберите другой сервер."
            }
            (_, NoCandidates) => {
                "Ни один проверенный сервер не подтвердил передачу данных. Повторите поиск или выберите другой сервер."
            }
            _ => "Подключение не подтвердило передачу данных через WARP. Повторите попытку или выберите другой сервер.",
        }
    }
}

impl std::fmt::Display for WarpFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for WarpFailure {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_cause_survives_context_without_exporting_sensitive_details() {
        let expected = WarpFailure::new(FailureStage::Cleanup, FailureReason::CleanupPending);
        let error = anyhow::Error::new(expected).context("synthetic-token/private-key");
        let actual = WarpFailure::from_error(&error, FailureStage::Scanner, FailureReason::InvalidOutput);
        assert_eq!(actual, expected);
        assert!(!serde_json::to_string(&actual).unwrap().contains("synthetic"));
        assert!(!actual.to_string().contains("private-key"));
    }

    #[test]
    fn permission_denial_is_not_reported_as_an_unreachable_server() {
        let error = anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let failure = WarpFailure::from_error(&error, FailureStage::Runtime, FailureReason::Unreachable);
        assert_eq!(failure.reason, FailureReason::PermissionDenied);
        assert!(!failure.retryable);
    }
}
