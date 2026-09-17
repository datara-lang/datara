use crate::dmir::ir::*;
use crate::types::DataraType;

use super::Lowering;

impl<'a> Lowering<'a> {
    /// Lowers declarative bridge calls (`module.func(args)`).
    pub(crate) fn lower_bridge_call(
        &mut self,
        ns_name: &str,
        member: &str,
        arg_vals: &[ValueId],
        cur_block: &mut BasicBlockId,
    ) -> Option<ValueId> {
        let qualified = format!("{}.{}", ns_name, member);
        let meta = self
            .bridge_registry
            .lookup(&qualified)
            .or_else(|| self.bridge_registry.lookup(member))
            .cloned()?;

        let dest = self.next_val();
        if meta.lang == "py" {
            let fn_name_val = self.next_val();
            self.get_block_mut(*cur_block)
                .instructions
                .push(Inst::ConstStr {
                    dest: fn_name_val,
                    value: format!("{}.{}", meta.module, meta.fn_name),
                });
            if meta.param_types.len() == 1
                && meta.param_types[0] == DataraType::Float
                && meta.return_type == DataraType::Float
                && !arg_vals.is_empty()
            {
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_py_call_1_float".into(),
                        args: vec![fn_name_val, arg_vals[0]],
                        ty: "Float".into(),
                    });
                return Some(dest);
            }
            if meta.param_types.len() == 2
                && meta.param_types[0] == DataraType::Float
                && meta.param_types[1] == DataraType::Float
                && meta.return_type == DataraType::Float
                && arg_vals.len() >= 2
            {
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_py_call_2_float".into(),
                        args: vec![fn_name_val, arg_vals[0], arg_vals[1]],
                        ty: "Float".into(),
                    });
                return Some(dest);
            }
            if meta.param_types.len() == 2
                && meta.param_types[0] == DataraType::Float
                && meta.param_types[1] == DataraType::Float
                && meta.return_type == DataraType::Bool
                && arg_vals.len() >= 2
            {
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_py_call_2_float_bool".into(),
                        args: vec![fn_name_val, arg_vals[0], arg_vals[1]],
                        ty: "Bool".into(),
                    });
                return Some(dest);
            }
            if meta.param_types.len() == 1
                && meta.param_types[0] == DataraType::String
                && meta.return_type == DataraType::String
                && !arg_vals.is_empty()
            {
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_py_call_1_str".into(),
                        args: vec![fn_name_val, arg_vals[0]],
                        ty: "String".into(),
                    });
                return Some(dest);
            }
            if meta.param_types.len() == 1
                && meta.param_types[0] == DataraType::Int
                && meta.return_type == DataraType::Int
                && !arg_vals.is_empty()
            {
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_py_call_1_int".into(),
                        args: vec![fn_name_val, arg_vals[0]],
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if meta.param_types.len() == 2
                && meta.param_types[0] == DataraType::Int
                && meta.param_types[1] == DataraType::Int
                && meta.return_type == DataraType::Int
                && arg_vals.len() >= 2
            {
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_py_call_2_int".into(),
                        args: vec![fn_name_val, arg_vals[0], arg_vals[1]],
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if matches!(meta.return_type, DataraType::List(_)) && !arg_vals.is_empty() {
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_py_call_list_f64".into(),
                        args: vec![fn_name_val, arg_vals[0]],
                        ty: "List<Float>".into(),
                    });
                return Some(dest);
            }
            let ret_repr = match meta.return_type {
                DataraType::Float => "Float",
                DataraType::String => "String",
                DataraType::Bool => "Bool",
                DataraType::Unit => "Unit",
                _ => "Int",
            };
            self.get_block_mut(*cur_block)
                .instructions
                .push(Inst::Call {
                    dest,
                    func: "datara_py_call_1_str".into(),
                    args: if !arg_vals.is_empty() {
                        vec![fn_name_val, arg_vals[0]]
                    } else {
                        vec![fn_name_val]
                    },
                    ty: ret_repr.into(),
                });
            return Some(dest);
        }

        if meta.lang == "js" {
            let fn_name_val = self.next_val();
            self.get_block_mut(*cur_block)
                .instructions
                .push(Inst::ConstStr {
                    dest: fn_name_val,
                    value: format!("{}.{}", meta.module, meta.fn_name),
                });
            if arg_vals.len() == 1 {
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_js_call_1".into(),
                        args: vec![fn_name_val, arg_vals[0]],
                        ty: "String".into(),
                    });
                return Some(dest);
            }
            if arg_vals.len() == 2 {
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_js_call_2".into(),
                        args: vec![fn_name_val, arg_vals[0], arg_vals[1]],
                        ty: "String".into(),
                    });
                return Some(dest);
            }
            if arg_vals.is_empty() {
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_js_call_0".into(),
                        args: vec![fn_name_val],
                        ty: "String".into(),
                    });
                return Some(dest);
            }
        }

        // C / Rust / other foreign bridges lower via standard direct call
        let ret_repr = match meta.return_type {
            DataraType::Float => "Float",
            DataraType::String => "String",
            DataraType::Bool => "Bool",
            DataraType::Unit => "Unit",
            _ => "Int",
        };
        self.get_block_mut(*cur_block)
            .instructions
            .push(Inst::Call {
                dest,
                func: member.to_string(),
                args: arg_vals.to_vec(),
                ty: ret_repr.into(),
            });
        Some(dest)
    }
}
