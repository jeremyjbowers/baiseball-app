use openssl::ssl::{SslConnector, SslMethod};
use postgres::types::{FromSql, ToSql};
use postgres::{Client, NoTls, Transaction};
use postgres_openssl::MakeTlsConnector;

pub struct Row<'a>(&'a postgres::Row);

impl<'a> Row<'a> {
    pub fn get<I, T>(&self, idx: I) -> Result<T, postgres::Error>
    where
        I: postgres::row::RowIndex + std::fmt::Display,
        T: FromSql<'a>,
    {
        self.0.try_get(idx)
    }
}

pub trait OptionalRow<T> {
    fn optional(self) -> Result<Option<T>, postgres::Error>;
}

impl<T> OptionalRow<T> for Result<T, postgres::Error> {
    fn optional(self) -> Result<Option<T>, postgres::Error> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(err) if err.to_string().contains("unexpected number of rows") => Ok(None),
            Err(err) => Err(err),
        }
    }
}

use crate::error::{db_err, ApiError};

pub struct Db {
    pub client: Client,
    test_schema: Option<String>,
}

pub struct Stmt<'a> {
    db: &'a mut Db,
    sql: String,
}

impl Db {
    pub fn execute(
        &mut self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<u64, postgres::Error> {
        self.client.execute(sql, params)
    }

    pub fn batch(&mut self, sql: &str) -> Result<(), postgres::Error> {
        self.client.batch_execute(sql)
    }

    pub fn prepare<'a>(&'a mut self, sql: &str) -> Result<Stmt<'a>, postgres::Error> {
        Ok(Stmt {
            db: self,
            sql: sql.to_string(),
        })
    }

    pub fn query_row<T, F>(
        &mut self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        f: F,
    ) -> Result<T, postgres::Error>
    where
        F: FnOnce(&Row<'_>) -> Result<T, postgres::Error>,
    {
        let row = self.client.query_one(sql, params)?;
        f(&Row(&row))
    }

    pub fn transaction(&mut self) -> Result<Tx<'_>, postgres::Error> {
        Ok(Tx {
            inner: self.client.transaction()?,
        })
    }
}

impl Stmt<'_> {
    pub fn query_map<T, F>(
        &mut self,
        params: &[&(dyn ToSql + Sync)],
        mut map: F,
    ) -> Result<std::vec::IntoIter<Result<T, postgres::Error>>, postgres::Error>
    where
        F: FnMut(&Row<'_>) -> Result<T, postgres::Error>,
    {
        let rows = self.db.client.query(&self.sql, params)?;
        Ok(rows
            .iter()
            .map(|row| map(&Row(row)))
            .collect::<Vec<_>>()
            .into_iter())
    }
}

pub struct Tx<'a> {
    inner: Transaction<'a>,
}

impl Tx<'_> {
    pub fn execute(
        &mut self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<u64, postgres::Error> {
        self.inner.execute(sql, params)
    }

    pub fn query_row<T, F>(
        &mut self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        f: F,
    ) -> Result<T, postgres::Error>
    where
        F: FnOnce(&Row<'_>) -> Result<T, postgres::Error>,
    {
        let row = self.inner.query_one(sql, params)?;
        f(&Row(&row))
    }

    pub fn commit(self) -> Result<(), postgres::Error> {
        self.inner.commit()
    }

    pub fn query<T, F>(
        &mut self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        mut map: F,
    ) -> Result<Vec<T>, postgres::Error>
    where
        F: FnMut(&Row<'_>) -> Result<T, postgres::Error>,
    {
        let rows = self.inner.query(sql, params)?;
        rows.iter().map(|row| map(&Row(row))).collect()
    }
}

pub fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://league:league@127.0.0.1:5432/league_office?sslmode=disable".to_string()
    })
}

pub fn connect(url: &str) -> Result<Db, ApiError> {
    let wants_tls = url.contains("sslmode=require")
        || url.contains("sslmode=verify-ca")
        || url.contains("sslmode=verify-full");
    let client = if wants_tls {
        let builder = SslConnector::builder(SslMethod::tls()).map_err(|err| {
            eprintln!("tls: {err}");
            ApiError::internal()
        })?;
        let tls = MakeTlsConnector::new(builder.build());
        Client::connect(url, tls).map_err(db_err)?
    } else {
        Client::connect(url, NoTls).map_err(db_err)?
    };
    Ok(Db {
        client,
        test_schema: None,
    })
}

pub fn open() -> Result<Db, ApiError> {
    let mut db = connect(&database_url())?;
    migrate(&mut db)?;
    Ok(db)
}

#[cfg(test)]
pub fn open_test() -> Result<Db, ApiError> {
    let mut admin = connect(&database_url())?;
    let schema = format!("s{}", random_schema());
    admin
        .batch(&format!("CREATE SCHEMA {schema}"))
        .map_err(db_err)?;
    drop(admin);
    let mut db = connect(&database_url())?;
    db.batch(&format!("SET search_path TO {schema}"))
        .map_err(db_err)?;
    migrate(&mut db)?;
    db.test_schema = Some(schema);
    Ok(db)
}

impl Drop for Db {
    fn drop(&mut self) {
        if let Some(schema) = self.test_schema.clone() {
            let _ = self.batch(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"));
        }
    }
}

fn migrate(conn: &mut Db) -> Result<(), ApiError> {
    conn.batch(
        "
        CREATE TABLE IF NOT EXISTS owners (
          id TEXT PRIMARY KEY,
          email TEXT NOT NULL UNIQUE,
          phone TEXT UNIQUE,
          is_admin BOOLEAN NOT NULL DEFAULT FALSE,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS magic_links (
          token TEXT PRIMARY KEY,
          owner_id TEXT NOT NULL REFERENCES owners(id),
          expires_at TEXT NOT NULL,
          used_at TEXT,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions (
          token TEXT PRIMARY KEY,
          owner_id TEXT NOT NULL REFERENCES owners(id),
          expires_at TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS leagues (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          lineup_lock TEXT NOT NULL CHECK (lineup_lock IN ('game', 'week', 'half', 'series')),
          created_by TEXT NOT NULL REFERENCES owners(id),
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          league_type TEXT,
          season_length BIGINT,
          division_games BIGINT,
          outside_games BIGINT
        );
        CREATE TABLE IF NOT EXISTS subleagues (
          id TEXT PRIMARY KEY,
          league_id TEXT NOT NULL REFERENCES leagues(id),
          name TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE (league_id, name)
        );
        CREATE TABLE IF NOT EXISTS divisions (
          id TEXT PRIMARY KEY,
          league_id TEXT NOT NULL REFERENCES leagues(id),
          subleague_id TEXT REFERENCES subleagues(id),
          name TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS teams (
          id TEXT PRIMARY KEY,
          league_id TEXT NOT NULL REFERENCES leagues(id),
          division_id TEXT NOT NULL REFERENCES divisions(id),
          owner_id TEXT NOT NULL REFERENCES owners(id),
          name TEXT NOT NULL,
          home_park TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE (league_id, name)
        );
        CREATE TABLE IF NOT EXISTS lineups (
          id TEXT PRIMARY KEY,
          team_id TEXT NOT NULL REFERENCES teams(id),
          name TEXT NOT NULL,
          vs_lhp TEXT NOT NULL,
          vs_rhp TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE (team_id, name)
        );
        CREATE TABLE IF NOT EXISTS rivalries (
          id TEXT PRIMARY KEY,
          league_id TEXT NOT NULL REFERENCES leagues(id),
          team_id TEXT NOT NULL REFERENCES teams(id),
          opponent_team_id TEXT NOT NULL REFERENCES teams(id),
          games BIGINT NOT NULL,
          created_at TEXT NOT NULL,
          UNIQUE (league_id, team_id, opponent_team_id)
        );
        CREATE TABLE IF NOT EXISTS schedules (
          league_id TEXT PRIMARY KEY REFERENCES leagues(id),
          built_at TEXT NOT NULL,
          snapshot TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS schedule_games (
          id TEXT PRIMARY KEY,
          league_id TEXT NOT NULL REFERENCES leagues(id),
          round BIGINT NOT NULL,
          home_team_id TEXT NOT NULL REFERENCES teams(id),
          away_team_id TEXT NOT NULL REFERENCES teams(id),
          same_division BIGINT NOT NULL,
          rivalry BIGINT NOT NULL,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS team_managers (
          team_id TEXT NOT NULL REFERENCES teams(id),
          owner_id TEXT NOT NULL REFERENCES owners(id),
          position BIGINT NOT NULL,
          created_at TEXT NOT NULL,
          PRIMARY KEY (team_id, owner_id)
        );
        ",
    )
    .map_err(db_err)?;
    if !has_column(conn, "owners", "phone")? {
        conn.batch("ALTER TABLE owners ADD COLUMN phone TEXT UNIQUE;")
            .map_err(db_err)?;
    }
    if !has_column(conn, "owners", "is_admin")? {
        conn.batch("ALTER TABLE owners ADD COLUMN is_admin BOOLEAN NOT NULL DEFAULT FALSE;")
            .map_err(db_err)?;
    }
    if !has_column(conn, "leagues", "league_type")? {
        conn.batch(
            "
            ALTER TABLE leagues ADD COLUMN league_type TEXT;
            ALTER TABLE leagues ADD COLUMN season_length BIGINT;
            ALTER TABLE leagues ADD COLUMN division_games BIGINT;
            ALTER TABLE leagues ADD COLUMN outside_games BIGINT;
            ",
        )
        .map_err(db_err)?;
    }
    conn.batch(
        "
        INSERT INTO team_managers (team_id, owner_id, position, created_at)
        SELECT t.id, t.owner_id, 0, t.created_at
        FROM teams t
        WHERE NOT EXISTS (
          SELECT 1 FROM team_managers tm WHERE tm.team_id = t.id
        );
        ",
    )
    .map_err(db_err)?;
    if !has_column(conn, "divisions", "subleague_id")? {
        conn.batch("ALTER TABLE divisions ADD COLUMN subleague_id TEXT REFERENCES subleagues(id);")
            .map_err(db_err)?;
    }
    backfill_subleagues(conn)?;
    Ok(())
}

fn backfill_subleagues(conn: &mut Db) -> Result<(), ApiError> {
    conn.batch(
        "
        INSERT INTO subleagues (id, league_id, name, created_at, updated_at)
        SELECT
          'legacy-' || l.id,
          l.id,
          'League',
          COALESCE(
            (SELECT MIN(d.created_at) FROM divisions d WHERE d.league_id = l.id),
            to_char(timezone('utc', now()), 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
          ),
          COALESCE(
            (SELECT MIN(d.created_at) FROM divisions d WHERE d.league_id = l.id),
            to_char(timezone('utc', now()), 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
          )
        FROM leagues l
        WHERE EXISTS (
          SELECT 1 FROM divisions d WHERE d.league_id = l.id AND d.subleague_id IS NULL
        )
        AND NOT EXISTS (
          SELECT 1 FROM subleagues s WHERE s.league_id = l.id AND lower(s.name) = 'league'
        );

        UPDATE divisions d
        SET subleague_id = s.id
        FROM subleagues s
        WHERE d.league_id = s.league_id
          AND lower(s.name) = 'league'
          AND d.subleague_id IS NULL;
        ",
    )
    .map_err(db_err)?;
    if has_constraint(conn, "divisions", "divisions_league_id_name_key")? {
        conn.batch("ALTER TABLE divisions DROP CONSTRAINT divisions_league_id_name_key;")
            .map_err(db_err)?;
    }
    if !has_constraint(conn, "divisions", "divisions_subleague_id_name_key")? {
        conn.batch(
            "ALTER TABLE divisions ADD CONSTRAINT divisions_subleague_id_name_key UNIQUE (subleague_id, name);",
        )
        .map_err(db_err)?;
    }
    Ok(())
}

fn has_constraint(conn: &mut Db, table: &str, constraint: &str) -> Result<bool, ApiError> {
    let found = conn
        .query_row(
            "SELECT COUNT(*) FROM information_schema.table_constraints WHERE table_schema = current_schema() AND table_name = $1 AND constraint_name = $2",
            &[
                &table as &(dyn postgres::types::ToSql + Sync),
                &constraint as &(dyn postgres::types::ToSql + Sync),
            ],
            |row| row.get::<usize, i64>(0),
        )
        .map_err(db_err)?;
    Ok(found > 0)
}

fn has_column(conn: &mut Db, table: &str, column: &str) -> Result<bool, ApiError> {
    let found = conn
        .query_row(
            "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = $1 AND column_name = $2",
            &[
                &table as &(dyn postgres::types::ToSql + Sync),
                &column as &(dyn postgres::types::ToSql + Sync),
            ],
            |row| row.get::<usize, i64>(0),
        )
        .map_err(db_err)?;
    Ok(found > 0)
}

#[cfg(test)]
fn random_schema() -> String {
    use rand::Rng;
    let mut buffer = [0u8; 8];
    rand::thread_rng().fill(&mut buffer);
    buffer.iter().map(|byte| format!("{byte:02x}")).collect()
}
