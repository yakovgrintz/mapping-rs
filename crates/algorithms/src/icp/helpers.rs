use crate::{
    array,
    types::SameSizeMat,
    utils::{distance_squared, point_cloud::calculate_point_cloud_center},
    Sum,
};
use nalgebra::{
    ArrayStorage, ClosedAdd, ClosedDiv, ClosedSub, Const, Matrix, Point, Scalar, Vector,
};
use num_traits::{AsPrimitive, NumOps, Zero};

/// Calculates the Mean Squared Error between two point clouds.
/// # Generics
/// * T: either an [`f32`] or [`f64`]
///
/// # Arguments
/// * `transformed_points_a`: a slice of [`Point`], representing the source point cloud, transformed by the current [`Isometry`](nalgebra::Isometry) matrix.
/// * `points_b`: a slice of [`Point`], representing the point cloud to match against.
///
/// # Returns
/// A [`T`], representing the sum of all squared distances between each point in `transformed_points_a` and its corresponding point in `points_b`.
#[inline]
#[cfg_attr(feature = "tracing", tracing::instrument("Calculate MSE", skip_all))]
pub(crate) fn calculate_mse<T, const N: usize>(
    transformed_points_a: &[Point<T, N>],
    closest_points_in_b: &[Point<T, N>],
) -> T
where
    T: Copy + Default + NumOps + Scalar + Sum,
{
    transformed_points_a
        .iter()
        .zip(closest_points_in_b.iter())
        .map(|(transformed_a, closest_point_in_b)| {
            distance_squared(transformed_a, closest_point_in_b)
        })
        .sum()
}

/// Calculates the outer product of two `N` length [`Vector`]s.
///
/// # Arguments
/// * `point_a`: A reference to the first [`Vector`].
/// * `point_b`: A reference to the second [`Vector`].
///
/// # Returns
/// A [`SameSizeMat`] of size `N` by `N`, containing the outer product.
#[inline]
#[cfg_attr(
    feature = "tracing",
    tracing::instrument("Calculate Outer Product", skip_all)
)]
pub(crate) fn outer_product<T, const N: usize>(
    point_a: &Vector<T, Const<N>, ArrayStorage<T, N, 1>>,
    point_b: &Vector<T, Const<N>, ArrayStorage<T, N, 1>>,
) -> SameSizeMat<T, N>
where
    T: NumOps + Copy,
{
    Matrix::from_data(ArrayStorage(array::from_fn(|a_idx| {
        array::from_fn(|b_idx| point_a.data.0[0][a_idx] * point_b.data.0[0][b_idx])
    })))
}

/// Calculates the estimated transformation matrix between the two point clouds.
///
/// # Arguments
/// * `points_a`: a slice of [`Point`], representing the source point cloud.
/// * `closest_points`: a slice of [`Point`], representing the target nearest neighbour for each point in `points_a`.
///
/// # Returns
/// A tuple of
/// * [`SameSizeMat<T, N>`], representing the covariance matrix of the outer products of the centered point clouds.
/// * [`Point`], representing the `points_a` centeroid.
/// * [`Point`], representing the `closest_points` centeroid.
///
/// # Panics
/// See [`calculate_mean`]
#[inline]
#[cfg_attr(
    feature = "tracing",
    tracing::instrument("Estimate Transform And Means", skip_all)
)]
pub(crate) fn get_rotation_matrix_and_centeroids<T, const N: usize>(
    transformed_points_a: &[Point<T, N>],
    closest_points: &[Point<T, N>],
) -> (SameSizeMat<T, N>, Point<T, N>, Point<T, N>)
where
    T: ClosedAdd + ClosedDiv + ClosedSub + Copy + NumOps + Scalar + Zero,
    usize: AsPrimitive<T>,
{
    let (mean_transformed_a, mean_closest) = (
        calculate_point_cloud_center(transformed_points_a),
        calculate_point_cloud_center(closest_points),
    );

    let rot_mat = transformed_points_a.iter().zip(closest_points.iter()).fold(
        Matrix::from_array_storage(ArrayStorage([[T::zero(); N]; N])),
        |rot_mat, (transformed_point_a, closest_point)| {
            let a_distance_from_centeroid = transformed_point_a - mean_transformed_a;
            let closest_point_distance_from_centeroid = closest_point - mean_closest;
            rot_mat
                + outer_product(
                    &a_distance_from_centeroid,
                    &closest_point_distance_from_centeroid,
                )
        },
    );

    (rot_mat, mean_transformed_a, mean_closest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{Point3, Vector3};

    #[test]
    fn test_calculate_mean() {
        // Define a set of points
        let points: [Point<f64, 3>; 3] = [
            Point::from([1.0, 2.0, 3.0]),
            Point::from([4.0, 5.0, 6.0]),
            Point::from([7.0, 8.0, 9.0]),
        ];

        // Calculate mean
        let mean = calculate_point_cloud_center(&points);
        assert_eq!(
            mean,
            Point::from([4.0, 5.0, 6.0]),
            "The mean point was not calculated correctly."
        );
    }

    #[test]
    fn test_calculate_mse() {
        // Define two sets of points
        let transformed_points_a: [Point<f64, 3>; 3] = [
            Point::from([1.0, 2.0, 3.0]),
            Point::from([4.0, 4.0, 4.0]),
            Point::from([7.0, 7.0, 7.0]),
        ];

        let points_b: [Point<f64, 3>; 3] = [
            Point::from([1.0, 1.0, 1.0]),
            Point::from([4.0, 5.0, 6.0]),
            Point::from([8.0, 8.0, 8.0]),
        ];

        // Calculate MSE
        let mse = calculate_mse(&transformed_points_a, &points_b);

        assert_eq!(
            mse, 13.0,
            "The calculated MSE does not match the expected value."
        );
    }

    #[test]
    fn test_outer_product() {
        // Define two vectors
        let point_a = Vector3::new(1.0, 2.0, 3.0);
        let point_b = Vector3::new(4.0, 5.0, 6.0);

        // Calculate outer product
        let result = outer_product(&point_a, &point_b);
        assert_eq!(
            result,
            SameSizeMat::from_data(ArrayStorage([
                [4.0, 5.0, 6.0],
                [8.0, 10.0, 12.0],
                [12.0, 15.0, 18.0]
            ])),
            "The calculated outer product does not match the expected value."
        );
    }

    #[test]
    fn test_get_rotation_matrix_and_centeroids() {
        // Define two sets of points
        let points_a: [Point<f64, 3>; 3] = [
            Point::from([6.0, 4.0, 20.0]),
            Point::from([100.0, 60.0, 3.0]),
            Point::from([5.0, 20.0, 10.0]),
        ];

        let points_b: [Point<f64, 3>; 3] = [
            Point::from([40.0, 22.0, 12.0]),
            Point::from([10.0, 14.0, 10.0]),
            Point::from([7.0, 30.0, 20.0]),
        ];

        // Compute transform using centroids
        let (rot_mat, mean_a, mean_b) = get_rotation_matrix_and_centeroids(&points_a, &points_b);
        assert_eq!(
            mean_a,
            Point3::new(37.0, 28.0, 11.0),
            "The calculated mean of points_a does not match the expected value."
        );
        assert_eq!(
            mean_b,
            Point3::new(19.0, 22.0, 14.0),
            "The calculated mean of points_b does not match the expected value."
        );
        assert_eq!(
            rot_mat,
            Matrix::from_data(ArrayStorage([
                [-834.0, -760.0, -382.0],
                [-696.0, -320.0, -128.0],
                [273.0, 56.0, 8.0]
            ])),
            "The calculated rotation matrix does not match the expected value."
        );
    }
}
