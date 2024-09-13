pub enum FloatDirection {
    Up,
    Down,
}

pub fn shift_column_values(arr: &mut [u32], height: usize, direction: FloatDirection) {
    let width: usize = arr.len() / height;

    for col in 0..width {
        let column_vals: Vec<u32> =
            (0..height).map(|row| arr[row * width + col]).collect();

        let mut non_zero_vals: Vec<u32> =
            column_vals.iter().cloned().filter(|&x| x != 0).collect();

        let zero_count = height - non_zero_vals.len();
        match direction {
            FloatDirection::Up => {
                non_zero_vals.extend(vec![0; zero_count]);
            }
            FloatDirection::Down => {
                let mut new_vals = vec![0; zero_count];
                new_vals.extend(non_zero_vals);
                non_zero_vals = new_vals;
            }
        }

        for row in 0..height {
            arr[row * width + col] = non_zero_vals[row];
        }
    }
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    use super::*;

    #[test]
    fn test_shift_up_basic() {
        let mut arr = vec![
            0, 0, 0,
            1, 0, 0,
            0, 0, 0];
        let expected = vec![
            1, 0, 0,
            0, 0, 0,
            0, 0, 0];
        shift_column_values(&mut arr, 3, FloatDirection::Up);
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_shift_down_basic() {
        let mut arr = vec![
            0, 0, 0,
            1, 0, 0,
            0, 0, 0];
        let expected = vec![
            0, 0, 0,
            0, 0, 0,
            1, 0, 0];
        shift_column_values(&mut arr, 3, FloatDirection::Down);
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_shift_up_multiple_non_zero() {
        let mut arr = vec![
            0, 0, 0,
            1, 2, 0,
            3, 0, 0];
        let expected = vec![
            1, 2, 0,
            3, 0, 0,
            0, 0, 0];
        shift_column_values(&mut arr, 3, FloatDirection::Up);
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_shift_down_multiple_non_zero() {
        let mut arr = vec![
            0, 0, 0,
            1, 2, 0,
            3, 0, 0];
        let expected = vec![
            0, 0, 0,
            1, 0, 0,
            3, 2, 0];
        shift_column_values(&mut arr, 3, FloatDirection::Down);
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_shift_up_empty_column() {
        let mut arr = vec![
            0, 0, 0,
            0, 0, 0,
            0, 0, 0];
        let expected = vec![
            0, 0, 0,
            0, 0, 0,
            0, 0, 0];
        shift_column_values(&mut arr, 3, FloatDirection::Up);
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_shift_down_empty_column() {
        let mut arr = vec![
            0, 0, 0,
            0, 0, 0,
            0, 0, 0];
        let expected = vec![
            0, 0, 0,
            0, 0, 0,
            0, 0, 0];
        shift_column_values(&mut arr, 3, FloatDirection::Down);
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_shift_up_all_non_zero() {
        let mut arr = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let expected = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        shift_column_values(&mut arr, 3, FloatDirection::Up);
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_shift_down_all_non_zero() {
        let mut arr = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let expected = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        shift_column_values(&mut arr, 3, FloatDirection::Down);
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_shift_up_irregular_array() {
        let mut arr = vec![0, 0, 0, 0, 1, 2, 3, 0, 0, 0, 4, 0];
        let expected = vec![1, 2, 3, 0, 0, 0, 4, 0, 0, 0, 0, 0];
        shift_column_values(&mut arr, 3, FloatDirection::Up);
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_shift_down_irregular_array() {
        let mut arr = vec![
            0, 0, 0, 0,
            1, 2, 3, 0,
            0, 0, 4, 0];
        let expected = vec![
            0, 0, 0, 0,
            0, 0, 3, 0,
            1, 2, 4, 0];
        shift_column_values(&mut arr, 3, FloatDirection::Down);
        assert_eq!(arr, expected);
    }
}
