ALTER TABLE evaluation_snapshots
    DROP CONSTRAINT evaluation_snapshots_judgment_check,
    ADD CONSTRAINT evaluation_snapshots_judgment_check CHECK ((
        judgment IS NULL OR (
            status = 'validated_unpublished'
            AND jsonb_typeof(judgment) = 'object'
            AND judgment ?& ARRAY[
                'schema_version',
                'evaluation_version',
                'rubric_version',
                'status',
                'evidence_bundle_hash',
                'privacy',
                'judgments'
            ]
            AND judgment ->> 'schema_version' = '1.0.0'
            AND judgment ->> 'evaluation_version' = evaluation_version
            AND judgment ->> 'rubric_version' = rubric_version
            AND judgment ->> 'evidence_bundle_hash' = evidence_bundle_hash
            AND judgment ->> 'status' IN ('complete', 'partial')
            AND jsonb_typeof(judgment -> 'privacy') = 'object'
            AND (judgment -> 'privacy') ?& ARRAY[
                'evidence_scope',
                'external_transmission'
            ]
            AND judgment -> 'privacy' ->> 'evidence_scope'
                IN ('public_only', 'private_local')
            AND judgment -> 'privacy' ->> 'external_transmission'
                IN ('not_used', 'public_only', 'consented_private')
            AND (
                judgment -> 'privacy' ->> 'evidence_scope' <> 'private_local'
                OR judgment -> 'privacy' ->> 'external_transmission'
                    IN ('not_used', 'consented_private')
            )
            AND jsonb_typeof(judgment -> 'judgments') = 'array'
            AND jsonb_array_length(judgment -> 'judgments') > 0
        )
    ) IS TRUE);
