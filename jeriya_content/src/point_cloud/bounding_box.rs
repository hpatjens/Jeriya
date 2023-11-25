use jeriya_shared::nalgebra::Vector3;
use serde::{Deserialize, Serialize};

/// Axis-aligned bounding box defined by its minimum and maximum extent.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AABB {
    pub min: Vector3<f32>,
    pub max: Vector3<f32>,
}

impl AABB {
    /// Creates a new [`AABB`] that contains nothing (min: `f32::MAX`, max: `f32::MIN`).
    ///
    /// # Examples
    ///
    /// ```
    /// # use jeriya_content::point_cloud::bounding_box::AABB;
    /// let mut bounding_box = AABB::empty();
    /// assert!(bounding_box.is_empty());
    /// ```
    pub fn empty() -> Self {
        Self {
            min: Vector3::new(f32::MAX, f32::MAX, f32::MAX),
            max: Vector3::new(f32::MIN, f32::MIN, f32::MIN),
        }
    }

    /// Creates a new [`AABB`] that contains the given `points`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use jeriya_shared::nalgebra::Vector3;
    /// # use jeriya_content::point_cloud::bounding_box::AABB;
    /// # use jeriya_shared::float_cmp::assert_approx_eq;
    /// let bounding_box = AABB::from_slice(&[
    ///     Vector3::new(0.0, 0.0, 0.0),
    ///     Vector3::new(1.0, 2.0, 3.0),
    ///     Vector3::new(-4.0, -5.0, -6.0),
    /// ]);
    /// assert_approx_eq!(f32, bounding_box.min.x, -4.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.min.y, -5.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.min.z, -6.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.x, 1.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.y, 2.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.z, 3.0, ulps = 1);
    /// ```
    pub fn from_slice(points: &[Vector3<f32>]) -> Self {
        let mut bounding_box = Self::empty();
        bounding_box.extend(points);
        bounding_box
    }

    /// Inserts the given `point` into the [`AABB`] expanding it if necessary.
    ///
    /// # Examples
    ///
    /// ```
    /// # use jeriya_shared::nalgebra::Vector3;
    /// # use jeriya_content::point_cloud::bounding_box::AABB;
    /// # use jeriya_shared::float_cmp::assert_approx_eq;
    /// let mut bounding_box = AABB::empty();
    /// bounding_box.add(Vector3::new(0.0, 0.0, 0.0));
    /// assert!(!bounding_box.is_empty());
    ///
    /// bounding_box.add(Vector3::new(1.0, 2.0, 3.0));
    /// bounding_box.add(Vector3::new(-4.0, -5.0, -6.0));
    /// assert_approx_eq!(f32, bounding_box.min.x, -4.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.min.y, -5.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.min.z, -6.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.x, 1.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.y, 2.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.z, 3.0, ulps = 1);
    /// ```
    pub fn add(&mut self, point: Vector3<f32>) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
    }

    /// Checks whether the given point in contained in the `AABB`
    ///
    /// # Examples
    ///
    /// ```
    /// # use jeriya_shared::nalgebra::Vector3;
    /// # use jeriya_content::point_cloud::bounding_box::AABB;
    /// let mut bounding_box = AABB::empty();
    /// bounding_box.add(Vector3::new(1.0, 2.0, 3.0));
    /// bounding_box.add(Vector3::new(-4.0, -5.0, -6.0));
    /// assert!(bounding_box.contains(Vector3::new(0.0, 0.0, 0.0)));
    /// assert!(bounding_box.contains(Vector3::new(0.5, 0.5, 0.5)));
    /// assert!(bounding_box.contains(Vector3::new(1.0, 2.0, 3.0)));
    /// assert!(bounding_box.contains(Vector3::new(-4.0, -5.0, -6.0)));
    /// ```
    pub fn contains(&self, point: Vector3<f32>) -> bool {
        point >= self.min && point <= self.max
    }

    /// Returns the center of the `AABB`
    ///
    /// # Examples
    ///
    /// ```
    /// # use jeriya_shared::nalgebra::Vector3;
    /// # use jeriya_content::point_cloud::bounding_box::AABB;
    /// # use jeriya_shared::float_cmp::assert_approx_eq;
    /// let mut bounding_box = AABB::empty();
    /// bounding_box.add(Vector3::new(0.0, 0.0, 0.0));
    /// bounding_box.add(Vector3::new(1.0, 2.0, 3.0));
    /// bounding_box.add(Vector3::new(-4.0, -5.0, -6.0));
    /// assert_approx_eq!(f32, bounding_box.center().x, -1.5, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.center().y, -1.5, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.center().z, -1.5, ulps = 1);
    /// ```
    pub fn center(&self) -> Vector3<f32> {
        (self.min + self.max) / 2.0
    }

    /// Returns the size of the `AABB`
    pub fn size(&self) -> Vector3<f32> {
        self.max - self.min
    }

    /// Returns `true` if the `AABB` is empty.
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }
}

impl Default for AABB {
    fn default() -> Self {
        Self::empty()
    }
}

impl Extend<Vector3<f32>> for AABB {
    fn extend<T: IntoIterator<Item = Vector3<f32>>>(&mut self, iter: T) {
        for point in iter {
            self.add(point);
        }
    }
}

impl<'s> Extend<&'s Vector3<f32>> for AABB {
    fn extend<T: IntoIterator<Item = &'s Vector3<f32>>>(&mut self, iter: T) {
        for point in iter {
            self.add(*point);
        }
    }
}

#[cfg(test)]
mod tests {
    use jeriya_shared::float_cmp::assert_approx_eq;

    use super::*;

    #[test]
    fn smoke() {
        let mut bounding_box = AABB::empty();
        assert!(bounding_box.is_empty());
        bounding_box.add(Vector3::new(0.0, 0.0, 0.0));
        assert!(!bounding_box.is_empty());
        assert_approx_eq!(f32, bounding_box.min.x, 0.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.min.y, 0.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.min.z, 0.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.x, 0.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.y, 0.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.z, 0.0, ulps = 1);
    }

    #[test]
    fn expand_positive() {
        let mut bounding_box = AABB::empty();
        bounding_box.add(Vector3::new(0.0, 0.0, 0.0));
        bounding_box.add(Vector3::new(1.0, 2.0, 3.0));
        assert_approx_eq!(f32, bounding_box.min.x, 0.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.min.y, 0.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.min.z, 0.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.x, 1.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.y, 2.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.z, 3.0, ulps = 1);
    }

    #[test]
    fn expand_negative() {}

    #[test]
    fn extend() {
        let mut bounding_box = AABB::empty();
        let points = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(-4.0, -5.0, -6.0),
        ];
        bounding_box.extend(points);
        assert_approx_eq!(f32, bounding_box.min.x, -4.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.min.y, -5.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.min.z, -6.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.x, 1.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.y, 2.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.z, 3.0, ulps = 1);
    }

    #[test]
    fn extend_ref() {
        let mut bounding_box = AABB::empty();
        let points = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(-4.0, -5.0, -6.0),
        ];
        bounding_box.extend(points.iter());
        assert_approx_eq!(f32, bounding_box.min.x, -4.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.min.y, -5.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.min.z, -6.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.x, 1.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.y, 2.0, ulps = 1);
        assert_approx_eq!(f32, bounding_box.max.z, 3.0, ulps = 1);
    }
}
