use std::collections::HashMap;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};
use cptv_encoder::get_packed_frame_data;
use cptv_shared::v2::types::Cptv2Header;


#[pyfunction]
fn new_cptv(header_info: HashMap<String, String>) {
    // Do we need to return something here? A PyObject
}

#[pyfunction]
fn push_frame_header(frame_header: HashMap<String, String>) {

}

#[pyfunction]
fn push_frame_data<'py> (py: Python<'py>, prev_frame: Option<&[u8]>, curr_frame: &[u8], width: usize, height: usize) -> PyResult<&'py PyBytes>{
    let (bits_per_pixel, output) = get_packed_frame_data(prev_frame, curr_frame, width, height);
    Ok(PyBytes::new(py, &output))
}

/// Formats the sum of two numbers as string.
#[pyfunction]
fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
    Ok((a + b).to_string())
}

/// A Python module implemented in Rust. The name of this function must match
/// the `lib.name` setting in the `Cargo.toml`, else Python will not be able to
/// import the module.
#[pymodule]
fn encoder_py_bindings(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(push_frame_data, m)?)?;

    Ok(())
}
