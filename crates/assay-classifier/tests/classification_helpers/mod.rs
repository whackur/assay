mod attributes;
mod policy;

pub fn classify(path: &str) -> assay_classifier::FileClassification {
    let input = assay_classifier::FileClassificationInput::try_new(
        path,
        assay_classifier::LinguistAttributeFacts::available(None, None),
    )
    .expect("test paths must be portable");
    assay_classifier::BuiltInPolicy::V1.classify(&input)
}
