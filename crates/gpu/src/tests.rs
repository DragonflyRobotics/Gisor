use crate::execute_unit::ExecuteUnitClass;
use crate::inst_type::InstType;

#[test]
fn execute_unit_classifies_ptx_instructions() {
	assert_eq!(InstType::LdGlobalF32.execute_unit_class(), ExecuteUnitClass::Memory);
	assert_eq!(InstType::Ex2ApproxF32.execute_unit_class(), ExecuteUnitClass::Special);
	assert_eq!(InstType::AddS32.execute_unit_class(), ExecuteUnitClass::Generic);
}
