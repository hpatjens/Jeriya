use std::io::{self, Write};

use crate::aabb::AABB;

/// Writes the lines of a bounding box to an OBJ file.
///
/// The `vertex_index_offset` is the number of indices (l in obj) that have already
/// been written to the file. The number of newly written vertices is returned.
pub fn write_bounding_box_o(o_name: &str, vertex_offset: usize, mut obj_writer: impl Write, aabb: &AABB) -> io::Result<usize> {
    writeln!(obj_writer, "o {o_name}")?;

    // Write vertex positions
    writeln!(obj_writer, "v {} {} {}", aabb.min.x, aabb.min.y, aabb.min.z)?; // 1
    writeln!(obj_writer, "v {} {} {}", aabb.min.x, aabb.min.y, aabb.max.z)?; // 2
    writeln!(obj_writer, "v {} {} {}", aabb.min.x, aabb.max.y, aabb.min.z)?; // 3
    writeln!(obj_writer, "v {} {} {}", aabb.min.x, aabb.max.y, aabb.max.z)?; // 4
    writeln!(obj_writer, "v {} {} {}", aabb.max.x, aabb.min.y, aabb.min.z)?; // 5
    writeln!(obj_writer, "v {} {} {}", aabb.max.x, aabb.min.y, aabb.max.z)?; // 6
    writeln!(obj_writer, "v {} {} {}", aabb.max.x, aabb.max.y, aabb.min.z)?; // 7
    writeln!(obj_writer, "v {} {} {}", aabb.max.x, aabb.max.y, aabb.max.z)?; // 8

    // Write lines
    writeln!(obj_writer, "l {} {}", vertex_offset + 1, vertex_offset + 2)?;
    writeln!(obj_writer, "l {} {}", vertex_offset + 3, vertex_offset + 4)?;
    writeln!(obj_writer, "l {} {}", vertex_offset + 5, vertex_offset + 6)?;
    writeln!(obj_writer, "l {} {}", vertex_offset + 7, vertex_offset + 8)?;

    writeln!(obj_writer, "l {} {}", vertex_offset + 1, vertex_offset + 3)?;
    writeln!(obj_writer, "l {} {}", vertex_offset + 2, vertex_offset + 4)?;
    writeln!(obj_writer, "l {} {}", vertex_offset + 5, vertex_offset + 7)?;
    writeln!(obj_writer, "l {} {}", vertex_offset + 6, vertex_offset + 8)?;

    writeln!(obj_writer, "l {} {}", vertex_offset + 1, vertex_offset + 5)?;
    writeln!(obj_writer, "l {} {}", vertex_offset + 2, vertex_offset + 6)?;
    writeln!(obj_writer, "l {} {}", vertex_offset + 3, vertex_offset + 7)?;
    writeln!(obj_writer, "l {} {}", vertex_offset + 4, vertex_offset + 8)?;

    Ok(8)
}
