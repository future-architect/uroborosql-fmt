#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleLevel {
    Off,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleSetting {
    pub level: RuleLevel,
}
