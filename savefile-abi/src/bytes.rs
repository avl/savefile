use core::slice;
use bytes::buf::UninitSlice;
use bytes::BufMut;

extern crate savefile;
extern crate savefile_derive;
use savefile::prelude::{
    Schema, get_schema,
    Serializer, Serialize, Deserializer, Deserialize,
    SavefileError,ReadBytesExt, LittleEndian,
    ReceiverType, AbiMethodArgument, AbiMethod, AbiMethodInfo, AbiTraitDefinition,
};
use crate::{
    parse_return_value_impl,
    FlexBuffer, AbiExportable, TraitObject, AbiErrorMsg,
    RawAbiCallResult, AbiConnection, AbiConnectionMethod, AbiProtocol,
    abi_entry_light,
};
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::io::Cursor;
unsafe extern "C" fn abi_entry_light_buf_mut(flag: AbiProtocol) {
    unsafe {
        abi_entry_light::<dyn BufMut>(flag);
    }
}

unsafe impl AbiExportable for dyn BufMut {
    const ABI_ENTRY: unsafe extern "C" fn(flag: AbiProtocol) = abi_entry_light_buf_mut;
    fn get_definition(version: u32) -> AbiTraitDefinition {
        AbiTraitDefinition {
            name: "BufMut".to_string(),
            methods:
                vec![
                    AbiMethod {
                        name: "remaining_mut".to_string(),
                        info: AbiMethodInfo {
                            return_value: {
                                get_schema::<usize>(version)
                            },
                            receiver: ReceiverType::Shared,
                            arguments: vec![],
                            async_trait_heuristic: false,
                        },
                    },
                    AbiMethod {
                        name: "advance_mut".to_string(),
                        info: AbiMethodInfo {
                            return_value: {
                                get_schema::<()>(0)
                            },
                            receiver: ReceiverType::Mut,
                            arguments: vec![
                                    AbiMethodArgument {
                                        schema: {
                                            get_schema::<usize>(version)
                                        },
                                    },
                                ],
                            async_trait_heuristic: false,
                        },
                    },
                    AbiMethod {
                        name: "chunk_mut".to_string(),
                        info: AbiMethodInfo {
                            return_value: Schema::UninitSlice,
                            receiver: ReceiverType::Mut,
                            arguments: vec![],
                            async_trait_heuristic: false,
                        },
                    },
                ]
            ,
            sync: false,
            send: false,
        }
    }
    fn get_latest_version() -> u32 {
        0u32
    }
    fn call(
        trait_object: TraitObject,
        method_number: u16,
        effective_version: u32,
        _compatibility_mask: u64,
        data: &[u8],
        abi_result: *mut (),
        __savefile_internal_receiver: unsafe extern "C" fn(
            outcome: *const RawAbiCallResult,
            result_receiver: *mut (),
        ),
    ) -> Result<(), SavefileError> {
        let mut cursor = Cursor::new(data);
        let mut deserializer = Deserializer {
            file_version: cursor.read_u32::<LittleEndian>()?,
            reader: &mut cursor,
            ephemeral_state: HashMap::new(),
        };
        match method_number {
            0u16 => {
                let ret = unsafe { &*trait_object.as_const_ptr::<dyn BufMut>() }
                    .remaining_mut();
                let mut __savefile_internal_data = FlexBuffer::new();
                let mut serializer = Serializer {
                    writer: &mut __savefile_internal_data,
                    file_version: 0u32,
                };
                serializer.write_u32(effective_version)?;
                match ret.serialize(&mut serializer) {
                    Ok(()) => {
                        let outcome = RawAbiCallResult::Success {
                            data: __savefile_internal_data.as_ptr() as *const u8,
                            len: __savefile_internal_data.len(),
                        };
                        unsafe {
                            __savefile_internal_receiver(
                                &outcome as *const _,
                                abi_result,
                            )
                        }
                    }
                    Err(err) => {
                        let err_str = format!("{:?}", err);
                        let outcome = RawAbiCallResult::AbiError(AbiErrorMsg {
                            error_msg_utf8: err_str.as_ptr(),
                            len: err_str.len(),
                        });
                        unsafe {
                            __savefile_internal_receiver(
                                &outcome as *const _,
                                abi_result,
                            )
                        }
                    }
                }
            }
            1u16 => {
                let arg_cnt;
                arg_cnt = <usize as Deserialize>::deserialize(&mut deserializer)?;
                unsafe {
                    (&mut *trait_object.as_mut_ptr::<dyn BufMut>())
                        .advance_mut(arg_cnt)
                };
            }
            2u16 => {
                let ret = unsafe {
                    &mut *trait_object.as_mut_ptr::<dyn BufMut>()
                }
                    .chunk_mut();
                let mut __savefile_internal_data = FlexBuffer::new();
                let mut serializer = Serializer {
                    writer: &mut __savefile_internal_data,
                    file_version: 0u32,
                };
                serializer.write_u32(effective_version)?;

                match unsafe { serializer.write_raw_ptr_size(ret.as_mut_ptr(), ret.len()) } {
                    Ok(()) => {
                        let outcome = RawAbiCallResult::Success {
                            data: __savefile_internal_data.as_ptr() as *const u8,
                            len: __savefile_internal_data.len(),
                        };
                        unsafe {
                            __savefile_internal_receiver(
                                &outcome as *const _,
                                abi_result,
                            )
                        }
                    }
                    Err(err) => {
                        let err_str = format!("{:?}", err);
                        let outcome = RawAbiCallResult::AbiError(AbiErrorMsg {
                            error_msg_utf8: err_str.as_ptr(),
                            len: err_str.len(),
                        });
                        unsafe {
                            __savefile_internal_receiver(
                                &outcome as *const _,
                                abi_result,
                            )
                        }
                    }
                }
            }
            _ => {
                return Err(SavefileError::general("Unknown method number"));
            }
        }
        Ok(())
    }
}
unsafe impl BufMut for AbiConnection<dyn BufMut> {
    #[inline]
    fn remaining_mut(&self) -> usize {
        let info: &AbiConnectionMethod = &self.template.methods[0u16 as usize];
        let Some(callee_method_number) = info.callee_method_number else {
            panic!(
                    "Method \'{0}\' does not exist in implementation.",
                    info.method_name,
                );
        };
        let mut result_buffer = MaybeUninit::<
            Result<usize, SavefileError>,
        >::uninit();
        let compatibility_mask = info.compatibility_mask;
        let mut __savefile_internal_datarawdata = [0u8; 4usize];
        let mut __savefile_internal_data = Cursor::new(
            &mut __savefile_internal_datarawdata[..],
        );
        let mut serializer = Serializer {
            writer: &mut __savefile_internal_data,
            file_version: self.template.effective_version,
        };
        serializer.write_u32(self.template.effective_version).unwrap();
        unsafe {
            unsafe extern "C" fn abi_result_receiver<'async_trait>(
                outcome: *const RawAbiCallResult,
                result_receiver: *mut (),
            ) {
                let outcome = unsafe { &*outcome };
                let result_receiver = unsafe {
                    &mut *(result_receiver
                        as *mut std::mem::MaybeUninit<Result<usize, SavefileError>>)
                };
                result_receiver
                    .write(
                        parse_return_value_impl(
                            outcome,
                            |mut deserializer| -> Result<usize, SavefileError> {
                                Ok(<usize as Deserialize>::deserialize(&mut deserializer)?)
                            },
                        ),
                    );
            }
            (self
                .template
                .entry)(AbiProtocol::RegularCall {
                trait_object: self.trait_object,
                compatibility_mask,
                method_number: callee_method_number,
                effective_version: self.template.effective_version,
                data: __savefile_internal_datarawdata[..].as_ptr(),
                data_length: 4usize,
                abi_result: &mut result_buffer as *mut _ as *mut (),
                receiver: abi_result_receiver,
            });
        }
        let resval = unsafe { result_buffer.assume_init() };
        resval.expect("Unexpected panic in invocation target")
    }
    #[inline]
    unsafe fn advance_mut(&mut self, arg_cnt: usize) {
        let info: &AbiConnectionMethod = &self.template.methods[1u16 as usize];
        let Some(callee_method_number) = info.callee_method_number else {
            panic!(
                    "Method \'{0}\' does not exist in implementation.",
                    info.method_name,
                );
        };
        let mut result_buffer = MaybeUninit::<
            Result<(), SavefileError>,
        >::new(Ok(()));
        let compatibility_mask = info.compatibility_mask;
        let mut __savefile_internal_datarawdata = [0u8; 12usize];
        let mut __savefile_internal_data = Cursor::new(
            &mut __savefile_internal_datarawdata[..],
        );        let mut serializer = Serializer {
            writer: &mut __savefile_internal_data,
            file_version: self.template.effective_version,
        };
        serializer.write_u32(self.template.effective_version).unwrap();
        arg_cnt.serialize(&mut serializer).expect("Failed while serializing");
        debug_assert_eq!(std::mem::size_of_val(&arg_cnt), 8);
        unsafe {
            unsafe extern "C" fn abi_result_receiver(
                outcome: *const RawAbiCallResult,
                result_receiver: *mut (),
            ) {
                let outcome = unsafe { &*outcome };
                let result_receiver = unsafe {
                    &mut *(result_receiver
                        as *mut std::mem::MaybeUninit<Result<(), SavefileError>>)
                };
                result_receiver
                    .write(
                        parse_return_value_impl(
                            outcome,
                            |_| -> Result<(), SavefileError> { Ok(()) },
                        ),
                    );
            }
            (self
                .template
                .entry)(AbiProtocol::RegularCall {
                trait_object: self.trait_object,
                compatibility_mask,
                method_number: callee_method_number,
                effective_version: self.template.effective_version,
                data: __savefile_internal_datarawdata.as_ptr() as *const u8,
                data_length: 12,
                abi_result: &mut result_buffer as *mut _ as *mut (),
                receiver: abi_result_receiver,
            });
        }
        let resval = unsafe { result_buffer.assume_init() };
        resval.expect("Unexpected panic in invocation target")
    }
    #[inline]
    fn chunk_mut(&mut self) -> &mut UninitSlice {
        let info: &AbiConnectionMethod = &self.template.methods[2u16 as usize];
        let Some(callee_method_number) = info.callee_method_number else {
            panic!(
                    "Method \'{0}\' does not exist in implementation.",
                    info.method_name,
                );
        };
        let mut result_buffer = MaybeUninit::<
            Result<&mut UninitSlice, SavefileError>,
        >::uninit();
        let compatibility_mask = info.compatibility_mask;
        let mut __savefile_internal_datarawdata = [0u8; 4usize];
        let mut __savefile_internal_data = Cursor::new(
            &mut __savefile_internal_datarawdata[..],
        );
        let mut serializer = Serializer {
            writer: &mut __savefile_internal_data,
            file_version: self.template.effective_version,
        };
        serializer.write_u32(self.template.effective_version).unwrap();
        unsafe {
            unsafe extern "C" fn abi_result_receiver<'async_trait>(
                outcome: *const RawAbiCallResult,
                result_receiver: *mut (),
            ) {
                let outcome = unsafe { &*outcome };
                let result_receiver = unsafe {
                    &mut *(result_receiver
                        as *mut std::mem::MaybeUninit<
                        Result<&mut UninitSlice, SavefileError>,
                    >)
                };
                result_receiver
                    .write(
                        parse_return_value_impl(
                            outcome,
                            |deserializer| -> Result<&mut UninitSlice, SavefileError> {
                                let iptr: *mut u8 = deserializer.read_raw_ptr_mut()?;
                                let len = deserializer.read_usize()?;
                                let bytes = slice::from_raw_parts_mut(iptr, len);
                                Ok(
                                   UninitSlice::new(bytes),
                                )
                            },
                        ),
                    );
            }
            (self
                .template
                .entry)(AbiProtocol::RegularCall {
                trait_object: self.trait_object,
                compatibility_mask,
                method_number: callee_method_number,
                effective_version: self.template.effective_version,
                data: __savefile_internal_datarawdata[..].as_ptr(),
                data_length: 4usize,
                abi_result: &mut result_buffer as *mut _ as *mut (),
                receiver: abi_result_receiver,
            });
        }
        let resval = unsafe { result_buffer.assume_init() };
        resval.expect("Unexpected panic in invocation target")
    }
}

