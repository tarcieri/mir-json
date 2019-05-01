use rustc::hir::{self, Defaultness};
use rustc::hir::def_id::DefId;
use rustc::mir;
use rustc::mir::interpret;
use rustc::ty;
use rustc::ty::{TyCtxt};
use syntax::ast;
use serde_json;
use std::fmt::Write as FmtWrite;
use std::usize;

use analyz::to_json::*;

impl<'tcx, T> ToJson<'tcx> for ty::List<T>
    where
    T: ToJson<'tcx>,
{
    fn to_json(&self, mir: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        let mut j = Vec::new();
        for v in self {
            j.push(v.to_json(mir));
        }
        json!(j)
    }
}

basic_json_enum_impl!(ast::FloatTy);
basic_json_enum_impl!(ast::IntTy);
basic_json_enum_impl!(ast::UintTy);
basic_json_enum_impl!(hir::Mutability);
basic_json_enum_impl!(hir::def::CtorKind);
basic_json_enum_impl!(mir::Mutability);
basic_json_enum_impl!(mir::CastKind);

impl ToJson<'_> for mir::BorrowKind {
    fn to_json(&self, _mir: &mut MirState) -> serde_json::Value {
        match self {
            &mir::BorrowKind::Shared => json!("Shared"),
            &mir::BorrowKind::Shallow => json!("Shallow"),
            &mir::BorrowKind::Unique => json!("Unique"),
            &mir::BorrowKind::Mut{..} => json!("Mut"),
        }
    }
}

impl ToJson<'_> for ty::VariantDiscr {
    fn to_json(&self, mir: &mut MirState) -> serde_json::Value {
        match self {
            &ty::VariantDiscr::Relative(i) => {
                json!({"kind": "Relative", "index" : json!(i)})
            }
            &ty::VariantDiscr::Explicit(n) => {
                json!({"kind": "Explicit", "name" : n.to_json(mir)})
            }
        }
    }
}

impl ToJson<'_> for hir::def_id::DefId {
    fn to_json(&self, _mir: &mut MirState) -> serde_json::Value {
        json!(ty::tls::with(|tx| {
            let defpath = tx.def_path(*self);
            defpath.to_string_no_crate()
        }))
    }
}

// For type _references_. To translate ADT defintions, do it explicitly.
impl<'tcx> ToJson<'tcx> for ty::Ty<'tcx> {
    fn to_json(&self, mir: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        match &self.sty {
            &ty::TyKind::Bool => {
                json!({"kind": "Bool"})
            }
            &ty::TyKind::Char => {
                json!({"kind": "Char"})
            }
            &ty::TyKind::Int(ref t) => {
                json!({"kind": "Int", "intkind": t.to_json(mir)})
            }
            &ty::TyKind::Uint(ref t) => {
                json!({"kind": "Uint", "uintkind": t.to_json(mir)})
            }
            &ty::TyKind::Tuple(ref sl) => {
                json!({"kind": "Tuple", "tys": sl.to_json(mir)})
            }
            &ty::TyKind::Slice(ref f) => {
                json!({"kind": "Slice", "ty": f.to_json(mir)})
            }
            &ty::TyKind::Str => {
                json!({"kind": "Str"})
            }
            &ty::TyKind::Float(ref sz) => {
                json!({"kind": "Float", "size": sz.to_json(mir)})
            }
            &ty::TyKind::Array(ref t, ref size) => {
                json!({"kind": "Array", "ty": t.to_json(mir), "size": size.to_json(mir)})
            }
            &ty::TyKind::Ref(ref _region, ref ty, ref mtbl) => {
                json!({
                    "kind": "Ref",
                    "ty": ty.to_json(mir),
                    "mutability": mtbl.to_json(mir)
                })
            }
            &ty::TyKind::RawPtr(ref tm) => {
                json!({
                    "kind": "RawPtr",
                    "ty": tm.ty.to_json(mir),
                    "mutability": tm.mutbl.to_json(mir)
                })
            }
            &ty::TyKind::Adt(ref adtdef, ref substs) => {
                let did = adtdef.did;
                mir.used_types.insert(did);
                json!({
                    "kind": "Adt",
                    "name": defid_str(&did),
                    "substs": substs.to_json(mir)
                })
            }
            &ty::TyKind::FnDef(defid, ref substs) => {
                json!({
                    "kind": "FnDef",
                    "defid": defid.to_json(mir),
                    "substs": substs.to_json(mir)
                })
            }
            &ty::TyKind::Param(ref p) =>
                json!({"kind": "Param", "param": p.to_json(mir)}),
            &ty::TyKind::Closure(ref defid, ref closuresubsts) => {
                json!({
                    "kind": "Closure",
                    "defid": defid.to_json(mir),
                    "closuresubsts": closuresubsts.substs.to_json(mir)
                })
            }
            &ty::TyKind::Dynamic(ref bs, _) => {
                let did = bs.principal().skip_binder().def_id;
                json!({
                    "kind": "Dynamic",
                    "data": did.to_json(mir)
                    /*, "region": r.to_json(mir)*/
                })
            }
            &ty::TyKind::Projection(ref pty) => {
                json!({
                    "kind": "Projection",
                    "substs": pty.substs.to_json(mir),
                    "defid": pty.item_def_id.to_json(mir)
                })
            }
            &ty::TyKind::UnnormalizedProjection(ref pty) => {
                json!({
                    "kind": "UnnormalizedProjection",
                    "substs": pty.substs.to_json(mir),
                    "defid": pty.item_def_id.to_json(mir)
                })
            }
            &ty::TyKind::FnPtr(ref sig) => {
                json!({"kind": "FnPtr", "signature": sig.to_json(mir)})
            }
            &ty::TyKind::Never => {
                json!({"kind": "Never"})
            }
            &ty::TyKind::Error => {
                json!({"kind": "Error"})
            }
            &ty::TyKind::Infer(_) => {
                // TODO
                json!({"kind": "Infer"})
            }
            /*
            &ty::TyKind::Bound(_, _) => {
                // TODO
                json!({"kind": "Bound"})
            }
            */
            &ty::TyKind::Foreign(_) => {
                // TODO
                json!({"kind": "Foreign"})
            }
            &ty::TyKind::Generator(_, _, _) => {
                // TODO
                json!({"kind": "Generator"})
            }
            &ty::TyKind::GeneratorWitness(_) => {
                // TODO
                json!({"kind": "GeneratorWitness"})
            }
            &ty::TyKind::Opaque(_, _) => {
                // TODO
                json!({"kind": "Opaque"})
            }
        }
    }
}

impl ToJson<'_> for ty::ParamTy {
    fn to_json(&self, _mir: &mut MirState) -> serde_json::Value {
        json!(self.idx)
    }
}

impl<'tcx> ToJson<'tcx> for ty::PolyFnSig<'tcx> {
    fn to_json(&self, ms: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        // Note: I don't think we need binders in MIR, but we can change
        // this if we do.
        self.skip_binder().to_json(ms)
    }
}

impl<'tcx> ToJson<'tcx> for ty::FnSig<'tcx> {
    fn to_json(&self, ms: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        let input_jsons : Vec<serde_json::Value> =
            self.inputs().iter().map(|i| i.to_json(ms)).collect();
        json!({
            "inputs": input_jsons,
            "output": self.output().to_json(ms)
        })
    }
}

impl<'tcx> ToJson<'tcx> for ty::TraitRef<'tcx> {
    fn to_json(&self, ms: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        json!({
            "trait":  self.def_id.to_json(ms),
            "substs":  self.substs.to_json(ms)
        })
    }
}

impl<'tcx> ToJson<'tcx> for ty::PolyTraitPredicate<'tcx> {
    fn to_json(&self, ms: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        self.skip_binder().trait_ref.to_json(ms)
    }
}

impl<'tcx> ToJson<'tcx> for ty::ProjectionPredicate<'tcx> {
    fn to_json(&self, ms: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        json!({
            "projection_ty": self.projection_ty.to_json(ms),
            "ty": self.ty.to_json(ms)
        })
    }
}

impl<'tcx> ToJson<'tcx> for ty::ProjectionTy<'tcx> {
    fn to_json(&self, ms: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        json!({
            "substs": self.substs.to_json(ms),
            "item_def_id": self.item_def_id.to_json(ms)
        })
    }
}

impl<'tcx> ToJson<'tcx> for ty::Predicate<'tcx> {
    fn to_json(&self, ms: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        match self {
            &ty::Predicate::Trait(ref ptp) => {
                json!({
                    "trait_pred": ptp.to_json(ms)
                })
            }
            &ty::Predicate::Projection(ref ppp) => {
                json!({
                    "trait_proj": ppp.skip_binder().to_json(ms)
                })
            }
            _ => {
                json!("unknown_pred")
            }
        }
    }
}

impl<'tcx> ToJson<'tcx> for ty::GenericPredicates<'tcx> {
    fn to_json(&self, ms: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        fn gather_preds<'tcx>(
            ms: &mut MirState<'_, 'tcx>,
            preds: &ty::GenericPredicates<'tcx>,
            dest: &mut Vec<serde_json::Value>,
        ) {
            dest.extend(preds.predicates.iter().map(|p| p.0.to_json(ms)));
            if let Some(parent_id) = preds.parent {
                let parent_preds = ms.state.tcx.unwrap().predicates_of(parent_id);
                gather_preds(ms, &parent_preds, dest);
            }
        }

        let mut json_preds: Vec<serde_json::Value> = Vec::new();
        gather_preds(ms, self, &mut json_preds);
        json!({ "predicates": json_preds })
    }
}

impl ToJson<'_> for ty::GenericParamDef {
    fn to_json(&self, ms: &mut MirState) -> serde_json::Value {
        json!({
            "param_def": *(self.name.as_str()),
            "def_id": self.def_id.to_json(ms),
        }) // TODO
    }
}

impl ToJson<'_> for ty::Generics {
    fn to_json(&self, ms: &mut MirState) -> serde_json::Value {
        fn gather_params(
            ms: &mut MirState,
            generics: &ty::Generics,
            dest: &mut Vec<serde_json::Value>,
        ) {
            if let Some(parent_id) = generics.parent {
                let parent_generics = ms.state.tcx.unwrap().generics_of(parent_id);
                gather_params(ms, &parent_generics, dest);
            }
            dest.extend(generics.params.iter().map(|p| p.to_json(ms)));
        }

        let mut json_params: Vec<serde_json::Value> = Vec::new();
        gather_params(ms, self, &mut json_params);
        json!({
            "params": json_params
        }) // TODO
    }
}

pub fn assoc_item_json<'tcx>(
    ms: &mut MirState<'_, 'tcx>,
    tcx: &ty::TyCtxt<'_, 'tcx, 'tcx>,
    item: &ty::AssociatedItem
) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    let did = item.def_id;
    map.insert("name".to_owned(), did.to_json(ms));
    map.insert("generics".to_owned(), tcx.generics_of(did).to_json(ms));
    map.insert("predicates".to_owned(), tcx.predicates_of(did).to_json(ms));

    match item.kind {
        ty::AssociatedKind::Const => {
            map.insert("kind".to_owned(), json!("Const"));
            map.insert("type".to_owned(), tcx.type_of(did).to_json(ms));
        }
        ty::AssociatedKind::Method => {
            map.insert("kind".to_owned(), json!("Method"));
            map.insert("signature".to_owned(), tcx.fn_sig(did).to_json(ms));
        }
        ty::AssociatedKind::Type => {
            map.insert("kind".to_owned(), json!("Type"));
            match item.defaultness {
                Defaultness::Default { has_value: false } => {},
                Defaultness::Default { has_value: true } |
                Defaultness::Final => {
                    map.insert("type".to_owned(), tcx.type_of(did).to_json(ms));
                },
            };
        }
        ty::AssociatedKind::Existential => {
            map.insert("kind".to_owned(), json!("Existential"));
        }
    }

    if let ty::AssociatedItemContainer::ImplContainer(impl_did) = item.container {
        if let Some(trait_ref) = tcx.impl_trait_ref(impl_did) {
            let trait_did = trait_ref.def_id;
            for trait_item in tcx.associated_items(trait_did) {
                if trait_item.ident.name == item.ident.name {
                    map.insert("implements".to_owned(), trait_item.def_id.to_json(ms));
                }
            }
        }
    }

    map.into()
}

pub fn defid_str(d: &hir::def_id::DefId) -> String {
    ty::tls::with(|tx| {
        let defpath = tx.def_path(*d);
        defpath.to_string_no_crate()
    })
}

pub fn defid_ty(d: &hir::def_id::DefId, mir: &mut MirState) -> serde_json::Value {
    let tcx = mir.state.tcx.unwrap();
    tcx.type_of(*d).to_json(mir)
}

pub trait ToJsonAg {
    fn tojson<'tcx>(
        &self,
        mir: &mut MirState<'_, 'tcx>,
        substs: &ty::subst::Substs<'tcx>,
    ) -> serde_json::Value;
}

impl<'tcx> ToJson<'tcx> for ty::subst::Kind<'tcx> {
    fn to_json(&self, mir: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        match self.unpack() {
            ty::subst::UnpackedKind::Type(ref ty) => ty.to_json(mir),
            ty::subst::UnpackedKind::Lifetime(_) => json!({"kind": "Lifetime"})
        }
    }
}

fn do_const_eval<'tcx>(
    tcx: TyCtxt<'_, 'tcx, 'tcx>,
    def_id: DefId,
    substs: &'tcx ty::subst::Substs<'tcx>
) -> &'tcx ty::Const<'tcx> {
    let param_env = ty::ParamEnv::reveal_all();
    let instance = ty::Instance::resolve(tcx, param_env, def_id, substs).unwrap();
    let cid = interpret::GlobalId {
        instance,
        promoted: None,
    };
    tcx.const_eval(param_env.and(cid)).unwrap()
}

fn eval_array_len<'tcx>(
    tcx: TyCtxt<'_, 'tcx, 'tcx>,
    c: &'tcx ty::Const<'tcx>,
) -> usize {
    let evaluated = match c.val {
        interpret::ConstValue::Unevaluated(def_id, substs) => {
            do_const_eval(tcx, def_id, substs)
        },
        _ => c,
    };
    match evaluated.val {
        interpret::ConstValue::Scalar(interpret::Scalar::Bits { size, bits }) => {
            assert!(bits <= usize::MAX as u128);
            bits as usize
        },
        _ => panic!("impossible: array size is not a scalar?"),
    }
}

fn read_static_memory<'tcx>(
    tcx: TyCtxt<'_, 'tcx, 'tcx>,
    ptr: mir::interpret::Pointer,
    len: usize,
) -> &'tcx [u8] {
    let alloc = tcx.alloc_map.lock().unwrap_memory(ptr.alloc_id);
    let start = ptr.offset.bytes() as usize;
    let end = start + len;
    assert!(alloc.relocations.len() == 0);
    &alloc.bytes[start .. end]
}

fn render_constant<'tcx>(
    tcx: TyCtxt<'_, 'tcx, 'tcx>,
    ty: ty::Ty<'tcx>,
    scalar: Option<(u8, u128)>,
    ptr: Option<mir::interpret::Pointer>,
) -> Option<(&'static str, serde_json::Value)> {
    Some(match ty.sty {
        ty::TyKind::Int(_) => {
            let (size, bits) = scalar.expect("int const had non-scalar value?");
            let mut val = bits as i128;
            if bits & (1 << (size * 8 - 1)) != 0 && size < 128 / 8 {
                // Sign-extend to 128 bits
                val |= -1_i128 << (size * 8);
            }
            ("int_val", val.to_string().into())
        },
        ty::TyKind::Bool |
        ty::TyKind::Char |
        ty::TyKind::Uint(_) => {
            let (size, bits) = scalar.expect("uint const had non-scalar value?");
            ("int_val", bits.to_string().into())
        },
        ty::TyKind::Float(ty::layout::FloatTy::F32) => {
            let (size, bits) = scalar.expect("f32 const had non-scalar value?");
            let val = f32::from_bits(bits as u32);
            ("float_val", val.to_string().into())
        },
        ty::TyKind::Float(ty::layout::FloatTy::F64) => {
            let (size, bits) = scalar.expect("f64 const had non-scalar value?");
            let val = f64::from_bits(bits as u64);
            ("float_val", val.to_string().into())
        },

        // &str - for string literals
        ty::TyKind::Ref(_, &ty::TyS {
            sty: ty::TyKind::Str,
            ..
        }, hir::Mutability::MutImmutable) => {
            let ptr = ptr.expect("string const had non-fatpointer value (no ptr)?");
            let (_, len_bits) = scalar.expect("string const had non-fatpointer value (no len)?");
            assert!(len_bits <= usize::MAX as u128);
            let len = len_bits as usize;
            let mem = read_static_memory(tcx, ptr, len);
            ("str_val", mem.into())
        },

        // &[u8; _] - for bytestring literals
        ty::TyKind::Ref(_, &ty::TyS {
            sty: ty::TyKind::Array(&ty::TyS {
                sty: ty::TyKind::Uint(ast::UintTy::U8),
                ..
            }, len_const),
            ..
        }, hir::Mutability::MutImmutable) => {
            let len = eval_array_len(tcx, len_const);
            let ptr = ptr.expect("bytestring const had non-pointer value?");
            let mem = read_static_memory(tcx, ptr, len);
            ("bstr_val", mem.into())
        },

        _ => return None,
    })
}

impl<'tcx> ToJson<'tcx> for ty::Const<'tcx> {
    fn to_json(&self, mir: &mut MirState<'_, 'tcx>) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert("kind".to_owned(), "Const".into());
        map.insert("ty".to_owned(), self.ty.to_json(mir));

        match self.val {
            interpret::ConstValue::Unevaluated(def_id, substs) => {
                map.insert("initializer".to_owned(), json!({
                    "def_id": def_id.to_json(mir),
                    "substs": substs.to_json(mir),
                }));
            },
            _ => {},
        }

        let evaluated = match self.val {
            interpret::ConstValue::Unevaluated(def_id, substs) => {
                do_const_eval(mir.state.tcx.unwrap(), def_id, substs)
            },
            _ => self,
        };
        let rendered = match evaluated.val {
            interpret::ConstValue::Scalar(interpret::Scalar::Bits { size, bits }) => {
                render_constant(mir.state.tcx.unwrap(), self.ty, Some((size, bits)), None)
            },
            interpret::ConstValue::Scalar(interpret::Scalar::Ptr(ptr)) => {
                render_constant(mir.state.tcx.unwrap(), self.ty, None, Some(ptr))
            },
            interpret::ConstValue::ScalarPair(
                interpret::Scalar::Ptr(ptr),
                interpret::Scalar::Bits { size, bits }
            ) => {
                render_constant(mir.state.tcx.unwrap(), self.ty, Some((size, bits)), Some(ptr))
            },
            _ => None,
        };
        if let Some((key, val)) = rendered {
            map.insert(key.to_owned(), val);
        }

        map.into()
    }
}

impl<T> ToJsonAg for Vec<T>
where
    T: ToJsonAg,
{
    fn tojson<'tcx>(
        &self,
        mir: &mut MirState<'_, 'tcx>,
        substs: &ty::subst::Substs<'tcx>,
    ) -> serde_json::Value {
        let mut j = Vec::new();
        for v in self {
            j.push(v.tojson(mir, substs));
        }
        json!(j)
    }
}

pub fn is_adt_ak(ak: &mir::AggregateKind) -> bool {
    match ak {
        &mir::AggregateKind::Adt(_, _, _, _, _) => true,
        _ => false,
    }
}

impl ToJsonAg for ty::AdtDef {
    fn tojson<'tcx>(
        &self,
        mir: &mut MirState<'_, 'tcx>,
        substs: &ty::subst::Substs<'tcx>,
    ) -> serde_json::Value {
        json!({
            "name": defid_str(&self.did),
            "variants": self.variants.tojson(mir, substs)
        })
    }
}

impl ToJsonAg for ty::VariantDef {
    fn tojson<'tcx>(
        &self,
        mir: &mut MirState<'_, 'tcx>,
        substs: &ty::subst::Substs<'tcx>,
    ) -> serde_json::Value {
        json!({
            "name": defid_str(&self.did),
            "discr": self.discr.to_json(mir),
            "fields": self.fields.tojson(mir, substs),
            "ctor_kind": self.ctor_kind.to_json(mir)
        })
    }
}

impl ToJsonAg for ty::FieldDef {
    fn tojson<'tcx>(
        &self,
        mir: &mut MirState<'_, 'tcx>,
        substs: &ty::subst::Substs<'tcx>,
    ) -> serde_json::Value {
        json!({
            "name": defid_str(&self.did),
            "ty": defid_ty(&self.did, mir),
            "substs": substs.to_json(mir)
        })
    }
}

pub fn handle_adt_ag<'tcx>(
    mir: &mut MirState<'_, 'tcx>,
    ak: &mir::AggregateKind<'tcx>,
    opv: &Vec<mir::Operand<'tcx>>,
) -> serde_json::Value {
    match ak {
        &mir::AggregateKind::Adt(ref adt, variant, substs, _, _) => {
            json!({
                "adt": adt.tojson(mir, substs),
                "variant": variant, //.to_json(mir),
                "ops": opv.to_json(mir)
            })
        }
        _ => unreachable!("bad"),
    }
}
