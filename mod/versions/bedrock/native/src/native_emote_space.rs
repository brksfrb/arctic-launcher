//! Owned coordinate changes between two observed native model/pixel spaces.
//! No projection, hand-side guess, engine pointer or inferred actor scale is
//! used. Both model matrices map native pixels into the same world/view space.
type Matrix = [f32; 16];
type Wide = [f64; 16];
const IDENTITY: Matrix = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

#[derive(Clone, PartialEq)]
pub(crate) struct Space {
    from_root: Matrix,
    to_root: Matrix,
    identity: bool,
}
fn affine(matrix: &Wide, bound: f64) -> bool {
    matrix
        .iter()
        .all(|value| value.is_finite() && value.abs() <= bound)
        && [3, 7, 11]
            .iter()
            .all(|index| matrix[*index].abs() <= 0.000001)
        && (matrix[15] - 1.).abs() <= 0.000001
}
fn multiply(a: &Wide, b: &Wide) -> Option<Wide> {
    let result = std::array::from_fn(|index| {
        let row = index % 4;
        let column = index / 4;
        (0..4).map(|k| a[k * 4 + row] * b[column * 4 + k]).sum()
    });
    result
        .iter()
        .all(|value: &f64| value.is_finite())
        .then_some(result)
}
fn narrow(matrix: Wide) -> Option<Matrix> {
    if !affine(&matrix, 1_000_000.) {
        return None;
    }
    let result = matrix.map(|value| value as f32);
    affine(&result.map(f64::from), 1_000_000.).then_some(result)
}
fn close(a: &Wide, b: &Wide, translation: f64) -> bool {
    a.iter().zip(b).enumerate().all(|(index, (a, b))| {
        let tolerance = if index / 4 == 3 && index % 4 < 3 {
            translation
        } else {
            0.00001 * a.abs().max(b.abs()).max(1.)
        };
        (a - b).abs() <= tolerance
    })
}
fn inverse(matrix: &Wide) -> Option<Wide> {
    if !affine(matrix, 32_000_000.) {
        return None;
    }
    // Invert the affine basis separately. Including the camera-relative
    // translation in pivot scaling would reject a well-conditioned far model.
    let mut rows = [[0f64; 6]; 3];
    let mut norm = 0f64;
    for row in 0..3 {
        let mut size = 0.;
        for column in 0..3 {
            rows[row][column] = matrix[column * 4 + row];
            size += rows[row][column].abs();
        }
        rows[row][row + 3] = 1.;
        norm = norm.max(size);
    }
    if norm == 0. {
        return None;
    }
    for column in 0..3 {
        let mut pivot = column;
        for row in column + 1..3 {
            if rows[row][column].abs() > rows[pivot][column].abs() {
                pivot = row;
            }
        }
        if rows[pivot][column].abs() <= norm * 1e-12 {
            return None;
        }
        rows.swap(column, pivot);
        let divisor = rows[column][column];
        for value in &mut rows[column] {
            *value /= divisor;
        }
        let source = rows[column];
        for row in 0..3 {
            if row == column {
                continue;
            }
            let factor = rows[row][column];
            for item in 0..6 {
                rows[row][item] -= factor * source[item];
            }
        }
    }
    let inverse_norm = rows
        .iter()
        .map(|row| row[3..].iter().map(|value| value.abs()).sum::<f64>())
        .fold(0f64, f64::max);
    if !inverse_norm.is_finite() || norm * inverse_norm > 1_000_000. {
        return None;
    }
    let mut result = IDENTITY.map(f64::from);
    for row in 0..3 {
        for column in 0..3 {
            result[column * 4 + row] = rows[row][column + 3];
        }
        result[12 + row] = -(0..3)
            .map(|column| rows[row][column + 3] * matrix[12 + column])
            .sum::<f64>();
    }
    if !affine(&result, f64::MAX)
        || !close(
            &multiply(matrix, &result)?,
            &IDENTITY.map(f64::from),
            0.0000001,
        )
        || !close(
            &multiply(&result, matrix)?,
            &IDENTITY.map(f64::from),
            0.0000001,
        )
    {
        return None;
    }
    Some(result)
}
impl Space {
    pub(crate) fn between(root: &Matrix, child: &Matrix) -> Option<Self> {
        let root_wide = root.map(f64::from);
        let child_wide = child.map(f64::from);
        let root_inverse = inverse(&root_wide)?;
        let child_inverse = inverse(&child_wide)?;
        if root == child {
            return Some(Self {
                from_root: IDENTITY,
                to_root: IDENTITY,
                identity: true,
            });
        }
        let from_root = narrow(multiply(&child_inverse, &root_wide)?)?;
        let to_root = narrow(multiply(&root_inverse, &child_wide)?)?;
        let forward = from_root.map(f64::from);
        let backward = to_root.map(f64::from);
        // Validate the stored f32 conversions, not only their f64 precursors.
        // Reject singular, badly conditioned or excessively lossy conversions.
        if !close(&multiply(&child_wide, &forward)?, &root_wide, 0.0001)
            || !close(&multiply(&root_wide, &backward)?, &child_wide, 0.0001)
            || !close(
                &multiply(&forward, &backward)?,
                &IDENTITY.map(f64::from),
                0.002,
            )
            || !close(
                &multiply(&backward, &forward)?,
                &IDENTITY.map(f64::from),
                0.002,
            )
        {
            return None;
        }
        Some(Self {
            from_root,
            to_root,
            identity: false,
        })
    }
    pub(crate) fn identity(&self) -> bool {
        self.identity
    }
    /// Owned attachment poses are archived in root pixel coordinates. Preserve
    /// identity-space endpoint bits; no world/view translation is serialized.
    pub(crate) fn root_matrix(&self, child: &Matrix) -> Option<Matrix> {
        if !affine(&child.map(f64::from), 1_000_000.) {
            return None;
        }
        if self.identity {
            return Some(*child);
        }
        narrow(multiply(
            &self.to_root.map(f64::from),
            &child.map(f64::from),
        )?)
    }
    pub(crate) fn child_matrix(&self, root: &Matrix) -> Option<Matrix> {
        if !affine(&root.map(f64::from), 1_000_000.) {
            return None;
        }
        if self.identity {
            return Some(*root);
        }
        narrow(multiply(
            &self.from_root.map(f64::from),
            &root.map(f64::from),
        )?)
    }
    pub(crate) fn matches_anchor(&self, root: &Matrix, child: &Matrix) -> bool {
        if !affine(&root.map(f64::from), 1_000_000.) || !affine(&child.map(f64::from), 1_000_000.) {
            return false;
        }
        if self.identity {
            return root == child;
        }
        let Some(expected) = multiply(&self.from_root.map(f64::from), &root.map(f64::from)) else {
            return false;
        };
        close(&expected, &child.map(f64::from), 0.002)
    }
    /// A_child' = U * (T_root * inverse(R_root)) * inverse(U) * A_child.
    /// Work in f64 through the complete conjugation and round once for the
    /// native upload. A mirrored/non-uniform native basis is preserved.
    pub(crate) fn retarget(&self, delta: &Matrix, original: &Matrix) -> Option<Matrix> {
        let delta = delta.map(f64::from);
        let original = original.map(f64::from);
        if !affine(&delta, 1_000_000.) || !affine(&original, 1_000_000.) {
            return None;
        }
        narrow(multiply(
            &multiply(
                &multiply(&self.from_root.map(f64::from), &delta)?,
                &self.to_root.map(f64::from),
            )?,
            &original,
        )?)
    }
}
