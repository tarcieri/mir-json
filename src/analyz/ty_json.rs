use rustc::hir;
use rustc::mir::{self, Mir};
use rustc::ty;
use syntax::ast;
use serde_json;
use std::fmt::Write as FmtWrite;

use analyz::to_json::*;

impl<T> ToJson for ty::Slice<T>
    where
    T: ToJson,
{
    fn to_json(&self, mir: &Mir) -> serde_json::Value {
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
basic_json_enum_impl!(mir::BorrowKind);

impl ToJson for ty::VariantDiscr {
    fn to_json(&self, mir: &Mir) -> serde_json::Value {
        match self {
            &ty::VariantDiscr::Relative(i) => json!({"kind": "Relative", "index" : json!(i)}),
            &ty::VariantDiscr::Explicit(n) => json!({"kind": "Explicit", "name" : n.to_json(mir)}),
        }
    }
}

impl ToJson for hir::def_id::DefId {
    fn to_json(&self, _mir: &Mir) -> serde_json::Value {
        json!(ty::tls::with(|tx| {
            let defpath = tx.def_path(*self);
            defpath.to_string_no_crate()
        }))
    }
}

// For type _references_. To translate ADT defintions, do it explicitly.
impl<'a> ToJson for ty::Ty<'a> {
    fn to_json(&self, mir: &Mir) -> serde_json::Value {
        match &self.sty {
            &ty::TypeVariants::TyBool => json!({"kind": "Bool"}),
            &ty::TypeVariants::TyChar => json!({"kind": "Char"}),
            &ty::TypeVariants::TyInt(ref t) => json!({"kind": "Int", "intkind": t.to_json(mir)}),
            &ty::TypeVariants::TyUint(ref t) => json!({"kind": "Uint", "uintkind": t.to_json(mir)}),
            &ty::TypeVariants::TyTuple(sl, _) => json!({"kind": "Tuple", "tys": sl.to_json(mir)}),
            &ty::TypeVariants::TySlice(ref f) => json!({"kind": "Slice", "ty": f.to_json(mir)}),
            &ty::TypeVariants::TyStr => json!({"kind": "Str"}),
            &ty::TypeVariants::TyFloat(ref sz) => json!({"kind": "Float", "size": sz.to_json(mir)}),
            &ty::TypeVariants::TyArray(ref t, ref size) => {
                json!({"kind": "Array", "ty": t.to_json(mir), "size": size})
            }
            &ty::TypeVariants::TyRef(ref _region, ref tm) => {
                json!({"kind": "Ref", "ty": tm.ty.to_json(mir), "mutability": tm.mutbl.to_json(mir)})
            }
            &ty::TypeVariants::TyRawPtr(ref tm) => {
                json!({"kind": "RawPtr", "ty": tm.ty.to_json(mir), "mutability": tm.mutbl.to_json(mir)})
            }
            &ty::TypeVariants::TyAdt(ref adtdef, ref substs) => {
                json!({"kind": "Adt", "name": defid_str(&adtdef.did), "substs": substs.to_json(mir)})
            }
            &ty::TypeVariants::TyFnDef(defid, ref substs) => {
                json!({"kind": "FnDef", "defid": defid.to_json(mir), "substs": substs.to_json(mir)})
            }
            &ty::TypeVariants::TyParam(ref p) => json!({"kind": "Param", "param": p.to_json(mir)}),
            &ty::TypeVariants::TyClosure(ref defid, ref closuresubsts) => {
                json!({"kind": "Closure", "defid": defid.to_json(mir), "closuresubsts": closuresubsts.substs.to_json(mir)})
            }
            &ty::TypeVariants::TyDynamic(ref data, ..) => {
                json!({"kind": "Dynamic", "data": data.principal().map(|p| p.def_id()).to_json(mir) /*, "region": r.to_json(mir)*/ })
            }
            &ty::TypeVariants::TyProjection(..) => {
                // TODO
                json!({"kind": "Projection"})
            }
            &ty::TypeVariants::TyFnPtr(..) => {
                // TODO
                json!({"kind": "FnPtr" /* ,
                       "inputs": sig.inputs().to_json(mir)
                       "output": sig.output().to_json(mir)*/ })
            }
            &ty::TypeVariants::TyNever => {
                json!({"kind": "Never"})
            }
            &ty::TypeVariants::TyError => {
                json!({"kind": "Error"})
            }
            &ty::TypeVariants::TyAnon(_, _) => {
                // TODO
                panic!("Type not supported: TyAnon")
            }
            &ty::TypeVariants::TyInfer(_) => {
                // TODO
                panic!("Type not supported: TyInfer")
            }
        }
    }
}

impl ToJson for ty::ParamTy {
    fn to_json(&self, _mir: &Mir) -> serde_json::Value {
        json!(self.idx)
    }
}

pub fn defid_str(d: &hir::def_id::DefId) -> String {
    ty::tls::with(|tx| {
        let defpath = tx.def_path(*d);
        defpath.to_string(tx)
    })
}

pub fn defid_ty(d: &hir::def_id::DefId, mir: &Mir) -> serde_json::Value {
    ty::tls::with(|tx| tx.type_of(*d).to_json(mir))
}

trait ToJsonAg {
    fn tojson<'a, 'tcx>(
        &self,
        mir: &'a Mir<'a>,
        substs: &'tcx ty::subst::Substs<'tcx>,
    ) -> serde_json::Value;
}

impl<'a> ToJson for ty::subst::Kind<'a> {
    fn to_json(&self, mir: &Mir) -> serde_json::Value {
        self.as_type().to_json(mir)
    }
}

impl<T> ToJsonAg for Vec<T>
where
    T: ToJsonAg,
{
    fn tojson(&self, mir: &Mir, substs: &ty::subst::Substs) -> serde_json::Value {
        let mut j = Vec::new();
        for v in self {
            j.push(v.tojson(mir, substs));
        }
        json!(j)
    }
}

pub fn is_adt_ak(ak: &mir::AggregateKind) -> bool {
    match ak {
        &mir::AggregateKind::Adt(_, _, _, _) => true,
        _ => false,
    }
}

impl ToJsonAg for ty::AdtDef {
    fn tojson(&self, mir: &Mir, substs: &ty::subst::Substs) -> serde_json::Value {
        json!({"name": defid_str(&self.did), "variants": self.variants.tojson(mir, substs)})
    }
}

impl ToJsonAg for ty::VariantDef {
    fn tojson(&self, mir: &Mir, substs: &ty::subst::Substs) -> serde_json::Value {
        json!({"name": defid_str(&self.did), "discr": self.discr.to_json(mir), "fields": self.fields.tojson(mir, substs), "ctor_kind": self.ctor_kind.to_json(mir)})
    }
}

impl ToJsonAg for ty::FieldDef {
    fn tojson<'a, 'tcx>(
        &self,
        mir: &Mir<'a>,
        substs: &'tcx ty::subst::Substs<'tcx>,
    ) -> serde_json::Value {
        json!({"name": defid_str(&self.did), "ty": defid_ty(&self.did, mir), "substs": substs.to_json(mir)  })
    }
}

pub fn handle_adt_ag(
    mir: &Mir,
    ak: &mir::AggregateKind,
    opv: &Vec<mir::Operand>,
) -> serde_json::Value {
    match ak {
        &mir::AggregateKind::Adt(ref adt, variant, substs, _) => {
            json!({"adt": adt.tojson(mir, substs), "variant": variant, "ops": opv.to_json(mir)})
        }
        _ => unreachable!("bad"),
    }
}
