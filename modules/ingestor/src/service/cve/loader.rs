use crate::graph::Graph;
use crate::service::{cve::cve_record::v5::CveRecord, hashing::HashingRead, Error};
use std::io::Read;

/// Loader capable of parsing a CVE Record JSON file
/// and manipulating the Graph to integrate it into
/// the knowledge base.
///
/// Should result in ensuring that a *vulnerability*
/// related to the CVE Record exists in the fetch, _along with_
/// also ensuring that the CVE *advisory* ends up also
/// in the fetch.
pub struct CveLoader<'g> {
    graph: &'g Graph,
}

impl<'g> CveLoader<'g> {
    pub fn new(graph: &'g Graph) -> Self {
        Self { graph }
    }

    pub async fn load<L: Into<String>, R: Read>(
        &self,
        location: L,
        record: R,
    ) -> Result<(), Error> {
        let mut reader = HashingRead::new(record);
        let cve: CveRecord = serde_json::from_reader(&mut reader)?;

        let tx = self.graph.transaction().await?;

        let vulnerability = self
            .graph
            .ingest_vulnerability(cve.cve_metadata.cve_id(), &tx)
            .await?;

        vulnerability
            .set_title(cve.containers.cna.title.clone(), &tx)
            .await?;

        for description in cve.containers.cna.descriptions {
            vulnerability
                .add_description(&description.lang, &description.value, &tx)
                .await?;
        }

        let digests = reader.finish().map_err(|e| Error::Generic(e.into()))?;
        let encoded_sha256 = hex::encode(digests.sha256);

        let advisory = self
            .graph
            .ingest_advisory(cve.cve_metadata.cve_id(), location, encoded_sha256, (), &tx)
            .await?;

        // Link the advisory to the backing vulnerability
        advisory
            .link_to_vulnerability(cve.cve_metadata.cve_id(), &tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::graph::Graph;
    use test_context::test_context;
    use test_log::test;
    use trustify_common::db::{test::TrustifyContext, Transactional};

    use crate::service::cve::loader::CveLoader;

    #[test_context(TrustifyContext, skip_teardown)]
    #[test(tokio::test)]
    async fn cve_loader(ctx: TrustifyContext) -> Result<(), anyhow::Error> {
        let db = ctx.db;
        let graph = Graph::new(db);
        let data = include_bytes!("../../../../../etc/test-data/mitre/CVE-2024-28111.json");

        let loaded_vulnerability = graph.get_vulnerability("CVE-2024-28111", ()).await?;
        assert!(loaded_vulnerability.is_none());

        let loaded_advisory = graph
            .get_advisory(
                "06908108e8097f2a56e628e7814a7bd54a5fc95f645b7c9fab02c1eb8dd9cc0c",
                Transactional::None,
            )
            .await?;
        assert!(loaded_advisory.is_none());

        let loader = CveLoader::new(&graph);
        loader.load("CVE-2024-28111.json", &data[..]).await?;

        let loaded_vulnerability = graph.get_vulnerability("CVE-2024-28111", ()).await?;
        assert!(loaded_vulnerability.is_some());

        let loaded_advisory = graph
            .get_advisory(
                "06908108e8097f2a56e628e7814a7bd54a5fc95f645b7c9fab02c1eb8dd9cc0c",
                Transactional::None,
            )
            .await?;
        assert!(loaded_advisory.is_some());

        let loaded_vulnerability = loaded_vulnerability.unwrap();
        let descriptions = loaded_vulnerability.descriptions("en", ()).await?;
        assert_eq!(1, descriptions.len());
        assert!(descriptions[0]
            .starts_with("Canarytokens helps track activity and actions on a network"));

        Ok(())
    }
}
