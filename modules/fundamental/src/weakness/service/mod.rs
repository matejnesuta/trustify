use crate::weakness::model::{PaginatedWeaknessSummary, WeaknessDetails, WeaknessSummary};
use crate::Error;
use sea_orm::{EntityTrait, TransactionTrait};
use trustify_common::db::limiter::LimiterTrait;
use trustify_common::db::query::{Filtering, Query};
use trustify_common::db::Database;
use trustify_common::model::Paginated;
use trustify_entity::weakness;

pub struct WeaknessService {
    db: Database,
}

impl WeaknessService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list_weaknesses(
        &self,
        query: Query,
        paginated: Paginated,
    ) -> Result<PaginatedWeaknessSummary, Error> {
        let tx = self.db.begin().await?;
        let tx = (&tx).into();

        let limiter = weakness::Entity::find().filtering(query)?.limiting(
            &self.db,
            paginated.offset,
            paginated.limit,
        );

        let total = limiter.total().await?;
        let items = limiter.fetch().await?;

        Ok(PaginatedWeaknessSummary {
            items: WeaknessSummary::from_entities(&items, &tx).await?,
            total,
        })
    }

    pub async fn get_weakness(&self, id: &str) -> Result<Option<WeaknessDetails>, Error> {
        let tx = self.db.begin().await?;
        let tx = (&tx).into();

        if let Some(found) = weakness::Entity::find_by_id(id).one(&self.db).await? {
            Ok(Some(WeaknessDetails::from_entity(&found, &tx).await?))
        } else {
            Ok(None)
        }
    }
}
