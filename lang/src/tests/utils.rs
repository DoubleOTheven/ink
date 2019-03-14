use crate::contract_gen_impl2;

pub fn assert_eq_tokenstreams(
    input: proc_macro2::TokenStream,
    expected: proc_macro2::TokenStream,
) {
    assert_eq!(
        contract_gen_impl2(input)
            .map(|result| result.to_string())
            .map_err(|err| err.to_string()),
        Ok(expected.to_string())
    )
}
