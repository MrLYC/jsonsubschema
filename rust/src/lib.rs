mod dfa;

use pyo3::prelude::*;
use pyo3::exceptions::PyOverflowError;

use dfa::Dfa;

/// Parse a regex, intersect two, return the result as a regex string (or None if empty).
#[pyfunction]
fn regex_intersect(a: &str, b: &str) -> PyResult<Option<String>> {
    let da = Dfa::from_pattern(a).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    let db = Dfa::from_pattern(b).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    let inter = da.intersect(&db);
    let minimized = inter.minimize();
    if minimized.is_empty() {
        Ok(None)
    } else {
        Ok(Some(minimized.to_regex()))
    }
}

/// Check if L(a) ⊆ L(b).
#[pyfunction]
fn regex_is_subset(a: &str, b: &str) -> PyResult<bool> {
    let da = Dfa::from_pattern(a).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    let db = Dfa::from_pattern(b).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    // L(a) ⊆ L(b) iff L(a) ∩ L(¬b) = ∅
    let comp_b = db.complement();
    let diff = da.intersect(&comp_b);
    Ok(diff.is_empty())
}

/// Check if pattern matches the full string.
#[pyfunction]
fn regex_matches(pattern: &str, s: &str) -> PyResult<bool> {
    let d = Dfa::from_pattern(pattern).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    Ok(d.matches(s))
}

/// Check if two regexes are equivalent (match exactly the same strings).
#[pyfunction]
fn regex_equivalent(a: &str, b: &str) -> PyResult<bool> {
    let da = Dfa::from_pattern(a).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    let db = Dfa::from_pattern(b).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    // Equivalent iff (a ∩ ¬b) ∪ (¬a ∩ b) = ∅
    let comp_a = da.complement();
    let comp_b = db.complement();
    let d1 = da.intersect(&comp_b);
    let d2 = comp_a.intersect(&db);
    Ok(d1.is_empty() && d2.is_empty())
}

/// Return cardinality of the language, or raise OverflowError if infinite.
#[pyfunction]
fn regex_cardinality(pattern: &str) -> PyResult<u64> {
    let d = Dfa::from_pattern(pattern).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    let minimized = d.minimize();
    match minimized.cardinality() {
        Some(n) => Ok(n),
        None => Err(PyOverflowError::new_err("infinite language")),
    }
}

/// Enumerate all strings if the language is finite. Raises OverflowError if infinite.
#[pyfunction]
fn regex_enumerate(pattern: &str) -> PyResult<Vec<String>> {
    let d = Dfa::from_pattern(pattern).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    let minimized = d.minimize();
    match minimized.enumerate_strings(10000) {
        Some(strings) => Ok(strings),
        None => Err(PyOverflowError::new_err("infinite or too many strings")),
    }
}

/// Check if a regex matches nothing.
#[pyfunction]
fn regex_is_empty(pattern: &str) -> PyResult<bool> {
    let d = Dfa::from_pattern(pattern).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    Ok(d.is_empty())
}

#[pymodule]
fn jsonsubschema_regex(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(regex_intersect, m)?)?;
    m.add_function(wrap_pyfunction!(regex_is_subset, m)?)?;
    m.add_function(wrap_pyfunction!(regex_matches, m)?)?;
    m.add_function(wrap_pyfunction!(regex_equivalent, m)?)?;
    m.add_function(wrap_pyfunction!(regex_cardinality, m)?)?;
    m.add_function(wrap_pyfunction!(regex_enumerate, m)?)?;
    m.add_function(wrap_pyfunction!(regex_is_empty, m)?)?;
    Ok(())
}
