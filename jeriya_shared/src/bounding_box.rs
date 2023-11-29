use nalgebra::Vector3;

use serde::{Deserialize, Serialize};

/// Trait that allows a type to expand an [`AABB`].
pub trait Include {
    fn expand_aabb(&self, aabb: &mut AABB);
}

impl Include for Vector3<f32> {
    fn expand_aabb(&self, aabb: &mut AABB) {
        aabb.min.x = aabb.min.x.min(self.x);
        aabb.min.y = aabb.min.y.min(self.y);
        aabb.min.z = aabb.min.z.min(self.z);
        aabb.max.x = aabb.max.x.max(self.x);
        aabb.max.y = aabb.max.y.max(self.y);
        aabb.max.z = aabb.max.z.max(self.z);
    }
}

impl Include for AABB {
    fn expand_aabb(&self, aabb: &mut AABB) {
        aabb.min.x = aabb.min.x.min(self.min.x);
        aabb.min.y = aabb.min.y.min(self.min.y);
        aabb.min.z = aabb.min.z.min(self.min.z);
        aabb.max.x = aabb.max.x.max(self.max.x);
        aabb.max.y = aabb.max.y.max(self.max.y);
        aabb.max.z = aabb.max.z.max(self.max.z);
    }
}

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

    /// Creates a new [`AABB`] that contains everything (min: `f32::NEG_INFINITY`, max: `f32::INFINITY`).
    ///
    /// # Examples
    ///
    /// ```
    /// # use jeriya_content::point_cloud::bounding_box::AABB;
    /// let mut bounding_box = AABB::infinity();
    /// assert!(bounding_box.contains(Vector3::new(0.0, 0.0, 0.0)));
    /// assert!(bounding_box.contains(Vector3::new(87624.38, -923771.95, 9102823.51)));
    /// assert!(!bounding_box.is_empty());
    /// ```
    pub fn infinity() -> Self {
        Self {
            min: Vector3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
            max: Vector3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
        }
    }

    /// Creates a new [`AABB`] with the given `min` and `max` extent.
    ///
    /// # Examples
    ///
    /// ```
    /// # use jeriya_content::point_cloud::bounding_box::AABB;
    /// let mut bounding_box = AABB::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 1.0, 1.0));
    /// assert!(!bounding_box.is_empty());
    /// ```
    pub fn new(min: Vector3<f32>, max: Vector3<f32>) -> Self {
        Self { min, max }
    }

    /// Creates a new [`AABB`] with the given `size` around the given `center`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use jeriya_content::point_cloud::bounding_box::AABB;
    /// let mut bounding_box = AABB::from_center_and_size(Vector3::zeros(), Vector3::new(1.0, 1.0, 1.0));
    /// assert!(!bounding_box.is_empty());
    /// ```
    pub fn from_center_and_size(center: Vector3<f32>, size: Vector3<f32>) -> Self {
        let half_size = size / 2.0;
        Self {
            min: center - half_size,
            max: center + half_size,
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
        Self::from_iter(points.iter())
    }

    /// Creates a new [`AABB`] that contains the `points` from the iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// # use jeriya_shared::nalgebra::Vector3;
    /// # use jeriya_content::point_cloud::bounding_box::AABB;
    /// # use jeriya_shared::float_cmp::assert_approx_eq;
    /// let vec = vec![
    ///     Vector3::new(0.0, 0.0, 0.0),
    ///     Vector3::new(1.0, 2.0, 3.0),
    ///     Vector3::new(-4.0, -5.0, -6.0),
    /// ];
    /// let bounding_box = AABB::from_iter(vec.iter());
    /// assert_approx_eq!(f32, bounding_box.min.x, -4.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.min.y, -5.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.min.z, -6.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.x, 1.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.y, 2.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.z, 3.0, ulps = 1);
    /// ```
    pub fn from_iter<'v>(points: impl IntoIterator<Item = &'v Vector3<f32>>) -> Self {
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
    ///
    /// // Inserting a point into an empty AABB expands it to contain the point.
    /// bounding_box.include(&Vector3::new(0.0, 0.0, 0.0));
    /// assert!(!bounding_box.is_empty());
    ///
    /// bounding_box.include(&Vector3::new(1.0, 2.0, 3.0));
    /// bounding_box.include(&Vector3::new(-4.0, -5.0, -6.0));
    /// assert_approx_eq!(f32, bounding_box.min.x, -4.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.min.y, -5.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.min.z, -6.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.x, 1.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.y, 2.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.z, 3.0, ulps = 1);
    ///
    /// // Inserting an AABB into the AABB expands it to contain the AABB.
    /// bounding_box.include(&AABB::from_slice(&[
    ///    Vector3::new(0.0, 0.0, 0.0),
    ///    Vector3::new(2.0, 3.0, 4.0),
    ///    Vector3::new(-5.0, -6.0, -7.0),
    /// ]));
    /// assert_approx_eq!(f32, bounding_box.min.x, -5.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.min.y, -6.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.min.z, -7.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.x, 2.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.y, 3.0, ulps = 1);
    /// assert_approx_eq!(f32, bounding_box.max.z, 4.0, ulps = 1);
    /// ```
    pub fn include(&mut self, other: &impl Include) {
        other.expand_aabb(self);
    }

    /// Checks whether the given point in contained in the `AABB`
    ///
    /// # Examples
    ///
    /// ```
    /// # use jeriya_shared::nalgebra::Vector3;
    /// # use jeriya_content::point_cloud::bounding_box::AABB;
    /// let mut bounding_box = AABB::empty();
    /// bounding_box.include(&Vector3::new(1.0, 2.0, 3.0));
    /// bounding_box.include(&Vector3::new(-4.0, -5.0, -6.0));
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
    /// bounding_box.include(&Vector3::new(0.0, 0.0, 0.0));
    /// bounding_box.include(&Vector3::new(1.0, 2.0, 3.0));
    /// bounding_box.include(&Vector3::new(-4.0, -5.0, -6.0));
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
            self.include(&point);
        }
    }
}

impl<'s> Extend<&'s Vector3<f32>> for AABB {
    fn extend<T: IntoIterator<Item = &'s Vector3<f32>>>(&mut self, iter: T) {
        for point in iter {
            self.include(point);
        }
    }
}

#[cfg(test)]
mod tests {
    use float_cmp::assert_approx_eq;

    use super::*;

    #[test]
    fn smoke() {
        let mut bounding_box = AABB::empty();
        assert!(bounding_box.is_empty());
        bounding_box.include(&Vector3::new(0.0, 0.0, 0.0));
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
        bounding_box.include(&Vector3::new(0.0, 0.0, 0.0));
        bounding_box.include(&Vector3::new(1.0, 2.0, 3.0));
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
