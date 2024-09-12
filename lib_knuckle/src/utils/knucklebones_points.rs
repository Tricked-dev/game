use super::occurences::count_occurrences;

pub(crate) fn calculate_knucklebones_points(board: &[u32], width: usize) -> Vec<u32> {
    let multiplication_table = [
        [1, 4, 9],
        [2, 8, 18],
        [3, 12, 27],
        [4, 16, 36],
        [5, 20, 45],
        [6, 24, 54],
    ];

    let mut columns = vec![vec![]; width];
    for (i, &value) in board.iter().enumerate() {
        columns[i % width].push(value);
    }

    let mut results = Vec::new();

    for column in columns {
        let mut total = 0;
        let occ = count_occurrences(&column);

        for (&key, &value) in occ.iter() {
            if key == 0 {
                continue;
            }
            if key > 6 {
                return vec![];
            }
            total += multiplication_table[key as usize - 1]
                .get(value as usize - 1)
                .unwrap_or(&0);
        }

        results.push(total);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test only 3x3 since thats what we do in the game
    #[test]
    fn test_knucklebones() {
        let points = calculate_knucklebones_points(&[1, 0, 0, 0, 0, 0, 0, 0, 0], 3);
        assert_eq!(points, vec![1, 0, 0]);
        let points = calculate_knucklebones_points(&[6, 1, 2, 6, 0, 0, 6, 0, 0], 3);
        assert_eq!(points, vec![54, 1, 2]);
        let points = calculate_knucklebones_points(&[0, 0, 0, 0, 0, 0, 0, 0, 0], 3);
        assert_eq!(points, vec![0, 0, 0]);
    }

    // The comment above was a lie i want some tests for it
    #[test]
    fn test_invalid_knucklebones_inputs() {
        let points = calculate_knucklebones_points(&[1, 1, 1, 1], 1);
        assert_eq!(points, vec![0]);
        let points = calculate_knucklebones_points(&[0, 0, 0, 0, 0, 0, 0], 2);
        assert_eq!(points, vec![0, 0]);
        let points = calculate_knucklebones_points(&[7], 1);
        assert_eq!(points, vec![]);
    }
}
