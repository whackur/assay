use assay_classifier::{
    BuiltInPolicy, ClassificationDecision, ClassificationPolicy, FileClassificationInput,
    PolicyVersion,
};

pub struct NamedPolicy(pub &'static str);

impl ClassificationPolicy for NamedPolicy {
    fn policy_version(&self) -> PolicyVersion {
        PolicyVersion::try_new(self.0).unwrap()
    }

    fn evaluate(&self, input: &FileClassificationInput) -> ClassificationDecision {
        BuiltInPolicy::V1.evaluate(input)
    }
}
