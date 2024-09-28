use crate::{pool_extractor::DatabaseConnection, UserCreateError};
use axum::Json;
use lib_knuckle::api_interfaces::{LeaderBoard, LeaderBoardEntry};

pub async fn leader_board(
    DatabaseConnection(conn): DatabaseConnection,
) -> Result<axum::Json<LeaderBoard>, UserCreateError> {
    let leader_board = conn
        .query(
            /* language=postgresql */
            "
SELECT
    p.name,
    COALESCE(SUM(
        CASE
            WHEN m.player1 = p.player_id THEN m.points_p1
            WHEN m.player2 = p.player_id THEN m.points_p2
        END
    ), 0) AS total_points,
    COUNT(DISTINCT m.match_id) AS total_games,
    COUNT(CASE WHEN m.winner = p.player_id THEN 1 END) AS total_wins
FROM
    players p
LEFT JOIN
    matches m ON p.player_id IN (m.player1, m.player2)
GROUP BY
    p.player_id
ORDER BY
    total_points DESC, total_wins DESC, total_games DESC, p.name;
            ",
            &[],
        )
        .await?;
    let leader_board = leader_board
        .iter()
        .map(|row| {
            let name: &str = row.get(0);
            let total_points: i64 = row.get(1);
            let total_games: i64 = row.get(2);
            let total_wins: i64 = row.get(3);
            LeaderBoardEntry {
                name: name.to_owned(),
                total_points: total_points as u32,
                total_games: total_games as u32,
                total_wins: total_wins as u32,
            }
        })
        .collect::<Vec<_>>();

    Ok(Json(LeaderBoard {
        total: leader_board.len() as u32,
        entries: leader_board,
    }))
}
