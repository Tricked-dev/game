use crate::{pool_extractor::Conn, UserCreateError};

pub async fn init_db(conn: Conn) -> Result<(), UserCreateError> {
    conn.simple_query(
        /* language=postgresql */
        "
      CREATE TABLE players (
            player_id UUID PRIMARY KEY,
            public_key BYTEA NOT NULL,
            secret_key BYTEA NOT NULL,
            name TEXT NOT NULL
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
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
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

    Ok(())
}
