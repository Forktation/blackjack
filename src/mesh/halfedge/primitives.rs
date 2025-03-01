use super::*;

pub struct Box;

impl Box {
    pub fn build(center: Vec3, size: Vec3) -> HalfEdgeMesh {
        let hsize = size * 0.5;

        let v1 = center + Vec3::new(-hsize.x, -hsize.y, -hsize.z);
        let v2 = center + Vec3::new(hsize.x, -hsize.y, -hsize.z);
        let v3 = center + Vec3::new(hsize.x, -hsize.y, hsize.z);
        let v4 = center + Vec3::new(-hsize.x, -hsize.y, hsize.z);

        let v5 = center + Vec3::new(-hsize.x, hsize.y, -hsize.z);
        let v6 = center + Vec3::new(-hsize.x, hsize.y, hsize.z);
        let v7 = center + Vec3::new(hsize.x, hsize.y, hsize.z);
        let v8 = center + Vec3::new(hsize.x, hsize.y, -hsize.z);

        /*
               // Top
               hem.add_quad(v1, v2, v3, v4);
               //Bottom
               hem.add_quad(v5, v6, v7, v8);
               // Front
               hem.add_quad(v5, v8, v2, v1);
               // Back
               hem.add_quad(v4, v3, v7, v6);
               // Left
               hem.add_quad(v6, v5, v1, v4);
               // Right
               hem.add_quad(v7, v3, v2, v8);
        */
        HalfEdgeMesh::build_from_polygons(
            &[v1, v2, v3, v4, v5, v6, v7, v8],
            &[
                &[0, 1, 2, 3],
                &[4, 5, 6, 7],
                &[4, 7, 1, 0],
                &[3, 2, 6, 5],
                &[5, 4, 0, 3],
                &[6, 2, 1, 7],
            ],
        )
        .expect("Cube construction should not fail")
    }
}

pub struct Quad;
impl Quad {
    pub fn build(center: Vec3, normal: Vec3, right: Vec3, size: Vec2) -> HalfEdgeMesh {
        let normal = normal.normalize();
        let right = right.normalize();
        let forward = normal.cross(right);

        let hsize = size * 0.5;

        let v1 = center + hsize.x * right + hsize.y * forward;
        let v2 = center - hsize.x * right + hsize.y * forward;
        let v3 = center - hsize.x * right - hsize.y * forward;
        let v4 = center + hsize.x * right - hsize.y * forward;

        HalfEdgeMesh::build_from_polygons(&[v1, v2, v3, v4], &[&[0, 1, 2, 3]])
            .expect("Quad construction should not fail")
    }
}
