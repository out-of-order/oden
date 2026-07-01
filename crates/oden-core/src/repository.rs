use sea_orm::{DatabaseConnection, DbErr, EntityTrait};

pub struct Repository {
    db: DatabaseConnection,
}

impl Repository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find_all<E>(&self) -> Result<Vec<E::Model>, DbErr>
    where
        E: EntityTrait,
    {
        E::find().all(&self.db).await
    }
}
