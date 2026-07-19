use std::{collections::VecDeque, io::Cursor};

use assay_github::{
    BlobAnalysisKey, BlobCacheLookup, BlobCacheState, BlobWorkItem, GitHubHttp, GitHubRequest,
    GitHubResponse, TreeSink, TreeSinkError,
};

mod revision;
mod revision_edge;
mod tree;

pub const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
pub const BLOB_A: &str = "89abcdef0123456789abcdef0123456789abcdef";
pub const BLOB_B: &str = "abcdef0123456789abcdef0123456789abcdef01";
pub const RULES: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

pub struct FakeHttp {
    pub responses: VecDeque<Result<GitHubResponse, assay_github::TransportError>>,
    pub requests: Vec<GitHubRequest>,
}

impl FakeHttp {
    pub fn new(responses: Vec<GitHubResponse>) -> Self {
        Self {
            responses: responses.into_iter().map(Ok).collect(),
            requests: Vec::new(),
        }
    }
}

impl GitHubHttp for FakeHttp {
    fn execute(
        &mut self,
        request: GitHubRequest,
    ) -> Result<GitHubResponse, assay_github::TransportError> {
        self.requests.push(request);
        self.responses.pop_front().expect("fixture response")
    }
}

pub fn response(status: u16, headers: &[(&str, &str)], body: &str) -> GitHubResponse {
    GitHubResponse::new(
        status,
        headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        Box::new(Cursor::new(body.as_bytes().to_vec())),
    )
}

pub fn rate_headers() -> [(&'static str, &'static str); 3] {
    [
        ("x-ratelimit-limit", "60"),
        ("x-ratelimit-remaining", "59"),
        ("x-ratelimit-reset", "2000000000"),
    ]
}

pub struct FakeBlobCache;

impl BlobCacheLookup for FakeBlobCache {
    fn lookup(&self, key: &BlobAnalysisKey) -> BlobCacheState {
        match key.blob().as_str() {
            BLOB_A => BlobCacheState::Hit,
            BLOB_B => BlobCacheState::Miss,
            _ => BlobCacheState::Unavailable,
        }
    }
}

#[derive(Default)]
pub struct RecordingSink(pub Vec<BlobWorkItem>);

impl TreeSink for RecordingSink {
    fn accept(&mut self, item: BlobWorkItem) -> Result<(), TreeSinkError> {
        self.0.push(item);
        Ok(())
    }
}

pub fn tree_body(truncated: bool) -> String {
    format!(
        r#"{{"sha":"{REVISION}","truncated":{truncated},"tree":[
          {{"path":"package.json","mode":"100644","type":"blob","sha":"{BLOB_A}","size":120}},
          {{"path":"packages/api/package.json","mode":"100644","type":"blob","sha":"{BLOB_B}","size":90}},
          {{"path":"packages/api/src/lib.ts","mode":"100644","type":"blob","sha":"1111111111111111111111111111111111111111","size":1000}},
          {{"path":"vendor/tool","mode":"160000","type":"commit","sha":"2222222222222222222222222222222222222222"}},
          {{"path":"docs","mode":"040000","type":"tree","sha":"3333333333333333333333333333333333333333"}}
        ]}}"#
    )
}
