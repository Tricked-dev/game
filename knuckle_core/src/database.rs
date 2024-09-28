use crate::{pool_extractor::Conn, UserCreateError};

pub async fn init_db(conn: Conn) -> Result<(), UserCreateError> {
    conn.simple_query(
        /* language=postgresql */
        "
      CREATE TABLE players (
            player_id UUID PRIMARY KEY,
            public_key BYTEA NOT NULL,
            secret_key BYTEA NOT NULL,
            name TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE (public_key)
        );


    ",
    )
    .await
    .ok();

    conn.simple_query(
        /* language=postgresql */
        "
  CREATE TABLE started_matches (
            match_id UUID PRIMARY KEY,
            seed BIGINT NOT NULL,
            time BIGINT NOT NULL,
            player1 UUID NOT NULL,
            player2 UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

            FOREIGN KEY (player1) REFERENCES players(player_id),
            FOREIGN KEY (player2) REFERENCES players(player_id),
            UNIQUE (seed, time)
        );
    ",
    )
    .await
    .ok();
    conn.simple_query(
        /* language=postgresql */
        "
  CREATE TABLE matches (
            match_id UUID PRIMARY KEY,
            seed BIGINT NOT NULL,
            time BIGINT NOT NULL,
            player1 UUID NOT NULL,
            player2 UUID NOT NULL,
            winner UUID,
            result TEXT NOT NULL,
            points_p1 SMALLINT NOT NULL,
            points_p2 SMALLINT NOT NULL,
            started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

            FOREIGN KEY (player1) REFERENCES players(player_id),
            FOREIGN KEY (player2) REFERENCES players(player_id),
            FOREIGN KEY (winner) REFERENCES players(player_id),
            UNIQUE (seed, time)
        );
    ",
    )
    .await
    .ok();

    conn.simple_query(
        /* language=postgresql */
        "
  CREATE TABLE moves (
            match_id UUID NOT NULL,
            player_id UUID NOT NULL,
            number SMALLINT NOT NULL,
            x SMALLINT NOT NULL,
            seq INT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

            UNIQUE (match_id, seq),
            FOREIGN KEY (match_id) REFERENCES matches(match_id),
            FOREIGN KEY (player_id) REFERENCES players(player_id)
        );
    ",
    )
    .await
    .ok();

    conn.simple_query(
        "CREATE TABLE queue_times (
        queue_time INTEGER NOT NULL,
        queue_id UUID NOT NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    )
    ",
    )
    .await
    .ok();

    macro_rules! create_hypertable {
        ($table:literal, $column:literal) => {
            conn.simple_query(&format!("SELECT create_hypertable('{}', by_range('{}'), if_not_exists => TRUE, migrate_data => TRUE)", $table, $column))
                .await
                .ok();
        };
    }

    create_hypertable!("moves", "created_at");
    create_hypertable!("matches", "completed_at");
    create_hypertable!("started_matches", "created_at");
    create_hypertable!("queue_times", "created_at");

    Ok(())
}
