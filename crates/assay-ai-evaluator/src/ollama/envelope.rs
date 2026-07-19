use serde::Deserialize;

use crate::api::Usage;

#[derive(Deserialize)]
pub(crate) struct ChatEnvelope {
    pub(crate) choices: Vec<ChatChoice>,
    pub(crate) usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChatChoice {
    #[serde(default)]
    #[serde(rename = "index")]
    pub(crate) _index: Option<u32>,
    pub(crate) message: ChatMessage,
    #[serde(default)]
    #[serde(rename = "finish_reason")]
    pub(crate) _finish_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChatMessage {
    #[serde(default)]
    #[serde(rename = "role")]
    pub(crate) _role: Option<String>,
    pub(crate) content: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl ChatUsage {
    pub(crate) fn validated(self) -> Option<Usage> {
        (self.prompt_tokens.checked_add(self.completion_tokens)? == self.total_tokens).then_some(
            Usage {
                prompt_tokens: self.prompt_tokens,
                completion_tokens: self.completion_tokens,
                total_tokens: self.total_tokens,
            },
        )
    }
}
