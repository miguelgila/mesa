#[cfg(feature = "headers")] // c.f. the `Cargo.toml` section
pub fn generate_headers() -> ::std::io::Result<()> {
    ::safer_ffi::headers::builder()
        .to_file("libmesa.h")?
        .generate()
}
pub mod csm {
    pub mod hsmgroup {
        use std::ffi::{CStr, CString};
        use std::os::raw::c_char;
        use serde_json::Value;
        use ::safer_ffi::prelude::*;
        use git2::IntoCString;
        use std::mem;

        // Protocol buffers
        use protobuf::Message;
        include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
        use terraform::{ProtoHSMGroup};

        use crate::{error::Error, hsm::group::shasta::http_client::post_member};

        // pub extern "C" fn C_mesa_tf_hsmgroup_read(shasta_token: repr_c::String,
        //                                shasta_base_url: repr_c::String,
        //                                shasta_root_cert: repr_c::Vec<u8>,
        //                                group_name_opt: repr_c::String) {
        // #[ffi_export]
        // pub fn C_mesa_tf_resource_hsmgroup_create(shasta_token: *const libc::c_char,
        //                                shasta_base_url: *const libc::c_char,
        //                                // shasta_root_cert: &[u8],
        //                                 shasta_root_cert: *const libc::c_char,
        //                                group_name_opt: *const libc::c_char) -> *const c_char {
        //
            // This function should call mesa hsm create function AND then add the xnames
            // if they were provided. Consequently, the xnames need to be passed as well.
        #[ffi_export]
         fn C_mesa_tf_hsmgroup_read(shasta_token: repr_c::String,
                                       shasta_base_url: repr_c::String,
                                       shasta_root_cert: repr_c::Vec<u8>,
                                       group_name_opt: repr_c::String) {

            // let token = unsafe { CStr::from_ptr(shasta_token) }.to_str().unwrap();
            // let url = unsafe { CStr::from_ptr(shasta_base_url) }.to_str().unwrap();
            // let cert = unsafe { CStr::from_ptr(shasta_root_cert) }.to_bytes();

            // let name = unsafe { CStr::from_ptr(group_name_opt) }.to_str().unwrap().to_string();
            // let name = name_cstr.to_str().unwrap();

            let result = "";//create(token, url, cert, Option::from(&name));
            let c_string = result.into_c_string().expect("Unable to convert result to a C string");
            // return c_string.into_raw();


            // let c_string = CString::new(cname.to_string()).expect("CString::new failed");
            // c_string.into_raw() // Move ownership to C
            // println!("Hello {}!", name);
        }
    }
}

