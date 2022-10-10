use pyo3::{pyclass, pymethods};
use pyo3::PyResult;
use serde_json::Value;
use valico::json_schema::Scope;

#[pyclass]
#[derive(Debug)]
pub struct Validator {
    scope: Scope,
    url: url::Url,
}



#[pymethods]
impl Validator {
    // todo: remove unwrap

    #[new]
    fn new(json_schema_str: &str) -> PyResult<Validator> {
        let json_schema: Value = serde_json::from_str(json_schema_str).unwrap();
        let mut scope = Scope::new();

        let url = url::Url::parse("http://schema-id").ok().unwrap();

        scope.compile_with_id(&url, json_schema, true).unwrap();

        Ok(Validator { scope, url })
    }

    fn validate(&self, json_value: &str) -> PyResult<Option<String>> {
        let value: Value = serde_json::from_str(json_value).unwrap();
        let validate_result = self.scope.resolve(&self.url).unwrap().validate(&value);
        if !validate_result.is_valid() {
            let errors = serde_json::to_string(&validate_result.errors).unwrap();
            return Ok(Some(errors))
        }
        Ok(None)
    }
}
