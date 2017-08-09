//! Terms structure and factory.

use std::sync::RwLock ;

use hashconsing::* ;

use common::* ;
use self::info::* ;

pub mod info ;
pub mod build ;

/// Types.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum Typ {
  /// Integers.
  Int,
  /// Booleans.
  Bool,
}
impl Typ {
  /// Type parser.
  #[allow(unused_variables)]
  pub fn parse(
    bytes: & [u8]
  ) -> ::nom::IResult<& [u8], Self, Error> {
    fix_error!(
      bytes,
      Error,
      alt_complete!(
        map!(tag!("Int"),  |_| Typ::Int)  |
        map!(tag!("Bool"), |_| Typ::Bool)
      )
    )
  }
  /// Default value of a type.
  pub fn default_val(& self) -> Val {
    match * self {
      Typ::Int => Val::I( Int::zero() ),
      Typ::Bool => Val::B( true ),
    }
  }
}
impl ::rsmt2::Sort2Smt for Typ {
  fn sort_to_smt2<Writer>(
    & self, w: &mut Writer
  ) -> SmtRes<()> where Writer: Write {
    smt_cast_io!( "while writing type as smt2" => write!(w, "{}", self) )
  }
}
impl_fmt!{
  Typ(self, fmt) {
    use instance::Typ::* ;
    match * self {
      Int => fmt.write_str("Int"),
      Bool => fmt.write_str("Bool"),
    }
  }
}


/// Values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Val {
  /// Boolean value.
  B(bool),
  /// Integer value.
  I(Int),
  /// No value (context was incomplete).
  N,
}
impl Val {
  /// Extracts a boolean value.
  pub fn to_bool(self) -> Res<Option<bool>> {
    match self {
      Val::B(b) => Ok( Some(b) ),
      Val::I(_) => bail!("expected boolean value, found integer"),
      Val::N => Ok(None),
    }
  }
  /// Extracts an integer value.
  pub fn to_int(self) -> Res<Option<Int>> {
    match self {
      Val::B(_) => bail!("expected integer value, found boolean"),
      Val::I(i) => Ok( Some(i) ),
      Val::N => Ok(None),
    }
  }
  /// Value parser.
  #[allow(unused_variables)]
  pub fn parse(
    bytes: & [u8]
  ) -> ::nom::IResult<& [u8], Self, Error> {
    use self::build::* ;
    fix_error!(
      bytes,
      Error,
      alt_complete!(
        map!( tag!("true"), |_| Val::B(true) ) |
        map!( tag!("false"), |_| Val::B(false) ) |
        map!( int, |i| Val::I(i) ) |
        do_parse!(
          char!('(') >>
          spc_cmt >> char!('-') >>
          spc_cmt >> value: int >>
          spc_cmt >> char!(')') >>
          ( Val::I(- value) )
        )
      )
    )
  }
}
impl_fmt!{
  Val(self, fmt) {
    match * self {
      Val::I(ref i) => write!(fmt, "{}", i),
      Val::B(b) => write!(fmt, "{}", b),
      Val::N => fmt.write_str("?"),
    }
  }
}
impl From<bool> for Val {
  fn from(b: bool) -> Val {
    Val::B(b)
  }
}
impl From<Int> for Val {
  fn from(i: Int) -> Val {
    Val::I( i.into() )
  }
}
impl From<usize> for Val {
  fn from(i: usize) -> Val {
    Val::I( i.into() )
  }
}
impl From<isize> for Val {
  fn from(i: isize) -> Val {
    Val::I( i.into() )
  }
}
impl From<u32> for Val {
  fn from(i: u32) -> Val {
    Val::I( i.into() )
  }
}
impl From<i32> for Val {
  fn from(i: i32) -> Val {
    Val::I( i.into() )
  }
}
impl From<u64> for Val {
  fn from(i: u64) -> Val {
    Val::I( i.into() )
  }
}
impl From<i64> for Val {
  fn from(i: i64) -> Val {
    Val::I( i.into() )
  }
}
macro_rules! try_val {
  (int $e:expr) => (
    if let Some(i) = $e.to_int()? { i } else {
      return Ok( Val::N )
    }
  ) ;
  (bool $e:expr) => (
    if let Some(b) = $e.to_bool()? { b } else {
      return Ok( Val::N )
    }
  ) ;
}



/// A real term.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum RTerm {
  /// A clause variable.
  Var(VarIdx),
  /// An integer.
  Int(Int),
  /// A boolean.
  Bool(bool),
  /// An operator application.
  App {
    /// The operator.
    op: Op,
    /// The arguments.
    args: Vec<Term>,
  },
}
impl RTerm {

  /// Write a real term using a special function to write variables.
  pub fn write<W, WriteVar>(
    & self, w: & mut W, write_var: WriteVar
  ) -> IoRes<()>
  where W: Write, WriteVar: Fn(& mut W, VarIdx) -> IoRes<()> {
    let mut stack = vec![
      (vec![self], "", "")
    // ^^^^^^^^^|  ^|  ^^~~~ termination string (written once vector's empty)
    //          |   |~~~~~~~ prefix string      (written before next element)
    //          |~~~~~~~~~~~ elements to write
    ] ;
    while let Some( (mut to_do, sep, end) ) = stack.pop() {
      use self::RTerm::* ;
      if let Some(term) = to_do.pop() {
        stack.push( (to_do, sep, end) ) ;
        match * term {
          Var(v) => {
            write!(w, "{}", sep) ? ;
            write_var(w, v) ?
          },
          Int(ref i) => write!(w, "{}{}", sep, i) ?,
          Bool(b) => write!(w, "{}{}", sep, b) ?,
          App { op, ref args } => {
            write!(w, "{}({}", sep, op) ? ;
            stack.push(
              (args.iter().rev().map(|t| t.get()).collect(), " ", ")")
            )
          },
        }
      } else { w.write_all( end.as_bytes() ) ? }
    }
    Ok(())
  }

  /// Term evaluation (int).
  pub fn int_eval(& self, model: & VarMap<Val>) -> Res< Option<Int> > {
    self.eval(model)?.to_int()
  }

  /// Term evaluation (bool).
  pub fn bool_eval(& self, model: & VarMap<Val>) -> Res< Option<bool> > {
    self.eval(model)?.to_bool()
  }

  /// True if the term is the constant `true`.
  pub fn is_true(& self) -> bool {
    if let RTerm::Bool(b) = * self { b } else { false }
  }
  /// True if the term is the constant `false`.
  pub fn is_false(& self) -> bool {
    if let RTerm::Bool(b) = * self { ! b } else { false }
  }


  /// Term evaluation.
  pub fn eval(& self, model: & VarMap<Val>) -> Res<Val> {
    use self::RTerm::* ;
    let mut current = self ;
    let mut stack = vec![] ;

    'eval: loop {

      // Go down applications.
      let mut evaled = match * current {
        App { op, ref args } => {
          current = & args[0] ;
          stack.push( (op, & args[1..], vec![]) ) ;
          continue 'eval
        },
        // Rest are leaves, going up.
        Var(v) => model[v].clone(),
        Int(ref i) => Val::I( i.clone() ),
        Bool(b) => Val::B(b),
      } ;

      // Go up.
      'go_up: loop {
        if let Some( (op, to_do, mut values) ) = stack.pop() {
          if to_do.is_empty() {
            values.push(evaled) ;
            evaled = op.eval(values).chain_err(
              || format!("while evaluating operator `{}`", op)
            ) ? ;
            continue 'go_up
          } else {
            // Going down the first element of `to_do`.
            current = & to_do[0] ;
            values.push(evaled) ;
            stack.push( (op, & to_do[1..], values) ) ;
            // Go down.
            continue 'eval
          }
        } else {
          // We are at the top level, done.
          return Ok(evaled)
        }
      }

    }
  }

  /// If the term's an integer constants, returns the value.
  pub fn int_val(& self) -> Option<Int> {
    if let RTerm::Int(ref i) = * self { Some( i.clone() ) } else { None }
  }

  /// The highest variable index appearing in the term.
  pub fn highest_var(& self) -> Option<VarIdx> {
    let mut to_do = vec![ self ] ;
    let mut max = None ;
    while let Some(term) = to_do.pop() {
      match * term {
        RTerm::Var(i) => max = Some(
          ::std::cmp::max( i, max.unwrap_or(0.into()) )
        ),
        RTerm::Int(_) => (),
        RTerm::Bool(_) => (),
        RTerm::App{ ref args, .. } => for arg in args {
          to_do.push(arg)
        },
      }
    }
    max
  }

  /// Returns the variable index if the term is a variable.
  pub fn var_idx(& self) -> Option<VarIdx> {
    match * self {
      RTerm::Var(i) => Some(i),
      _ => None,
    }
  }
}
impl_fmt!{
  RTerm(self, fmt) {
    let mut buf = Vec::with_capacity(250) ;
    self.write(& mut buf, |w, idx| write!(w, "v_{}", idx)).expect(
      "fatal error during real term pretty printing"
    ) ;
    let s = ::std::str::from_utf8(& buf).expect(
      "fatal error during real term pretty printing"
    ) ;
    write!(fmt, "{}", s)
  }
}

// impl<'a, WriteVar> ::rsmt2::Expr2Smt<WriteVar> for SWrap<'a>
// where WriteVar: Fn(VarIdx) -> & Val {
//   fn expr_to_smt2<Writer: Write>(
//     & self, w: & mut Writer, _: & ()
//   ) -> SmtRes<()> {
//     smt_cast_io!(
//       "writing sample as expression" => write!(
//         w, "|p_{} {}|", self.0, self.1.uid()
//       )
//     )
//   }
// }



/// Hash consed term.
pub type Term = HConsed<RTerm> ;











/// Top term, as they appear in clauses.
pub enum TTerm {
  /// A predicate application.
  P {
    /// Predicate applied.
    pred: PrdIdx,
    /// The arguments.
    args: VarMap<Term>,
  },
  /// Negation of something.
  N(Term),
  /// Just a term.
  T(Term),
}
impl TTerm {
  /// True if the term is equivalent to `true`.
  pub fn is_true(& self) -> bool {
    match * self {
      TTerm::N(ref t) => t.is_false(),
      TTerm::T(ref t) => t.is_true(),
      _ => false,
    }
  }
  /// True if the term is equivalent to `false`.
  pub fn is_false(& self) -> bool {
    match * self {
      TTerm::N(ref t) => t.is_true(),
      TTerm::T(ref t) => t.is_false(),
      _ => false,
    }
  }

  /// Writes a top term using special functions for writing predicates and
  /// variables.
  pub fn write<W, WriteVar, WritePrd>(
    & self, w: & mut W, write_var: WriteVar, write_prd: WritePrd
  ) -> IoRes<()>
  where
  W: Write,
  WriteVar: Fn(& mut W, VarIdx) -> IoRes<()>,
  WritePrd: Fn(& mut W, PrdIdx, & VarMap<Term>) -> IoRes<()> {
    use self::TTerm::* ;
    match * self {
      P { pred, ref args } => write_prd(w, pred, args),
      N(ref t) => {
        write!(w, "(not ") ? ;
        t.write(w, write_var) ? ;
        write!(w, ")", )
      },
      T(ref t) => t.write(w, write_var),
    }
  }

  /// Writes a top term smt2 style using a special function for writing
  /// predicates.
  pub fn write_smt2<W, WritePrd>(
    & self, w: & mut W, write_prd: WritePrd
  ) -> IoRes<()>
  where
  W: Write,
  WritePrd: Fn(& mut W, PrdIdx, & VarMap<Term>) -> IoRes<()> {
    self.write(
      w, |w, v| write!(w, "v{}", v), write_prd
    )
  }
}
impl_fmt!{
  TTerm(self, fmt) {
    match * self {
      TTerm::P { pred, ref args } => {
        write!(fmt, "(p_{}", pred) ? ;
        for arg in args {
          write!(fmt, " {}", arg) ?
        }
        write!(fmt, ")")
      },
      TTerm::N(ref t) => write!(fmt, "(not {})", t),
      TTerm::T(ref t) => write!(fmt, "{}", t),
    }
  }
}









/// A clause.
pub struct Clause {
  vars: VarMap<VarInfo>,
  lhs: Vec<TTerm>,
  rhs: TTerm,
}
impl Clause {
  /// Creates a clause.
  pub fn mk(
    vars: VarMap<VarInfo>, lhs: Vec<TTerm>, rhs: TTerm
  ) -> Self {
    Clause { vars, lhs, rhs }
  }

  /// LHS accessor.
  pub fn lhs(& self) -> & [TTerm] {
    & self.lhs
  }
  /// RHS accessor.
  pub fn rhs(& self) -> & TTerm {
    & self.rhs
  }

  /// Variables accessor.
  pub fn vars(& self) -> & VarMap<VarInfo> {
    & self.vars
  }

  /// Writes a clause given a special function to write predicates.  
  fn write<W, WritePrd>(
    & self, w: & mut W, write_prd: WritePrd
  ) -> IoRes<()>
  where W: Write, WritePrd: Fn(& mut W, PrdIdx, & VarMap<Term>) -> IoRes<()> {
    write!(w, "({}\n  (", ::common::consts::keywords::clause_def) ? ;
    for var in & self.vars {
      write!(w, " ({} {})", var.name, var.typ) ?
    }
    write!(w, " )\n  ") ? ;
    for_first!{
      self.lhs.iter() => {
        |fst| {
          write!(w, "( ") ? ;
          fst.write(
            w, |w, var| w.write_all( self.vars[var].as_bytes() ), & write_prd
          ) ?
        }, then |nxt| {
          write!(w, "\n    ") ? ;
          nxt.write(
            w, |w, var| w.write_all( self.vars[var].as_bytes() ), & write_prd
          ) ?
        }, yild write!(w, "\n  )") ?
      } else write!(w, "()") ?
    }

    write!(w, "\n  ") ? ;
    self.rhs.write(
      w, |w, var| w.write_all( self.vars[var].as_bytes() ), & write_prd
    ) ? ;
    write!(w, "\n)")
  }
}
impl ::std::ops::Index<VarIdx> for Clause {
  type Output = VarInfo ;
  fn index(& self, index: VarIdx) -> & VarInfo {
    & self.vars[index]
  }
}
impl ::rsmt2::Expr2Smt<Candidates> for Clause {
  fn expr_to_smt2<Writer: Write>(
    & self, writer: & mut Writer, cands: & Candidates
  ) -> SmtRes<()> {
    smt_cast_io!{
      "writing clause as smt2" =>
        write!(writer, "(not ") ;
        if self.lhs.is_empty() {
          Ok(())
        } else {
          write!(writer, "(=> (and")
        } ;
        {
          for lhs in & self.lhs {
            smtry_io!(
              "writing clause's lhs" =>
              write!(writer, " ") ;
              lhs.write_smt2(
                writer, |w, prd, args| {
                  cands[prd].write(
                    w, |w, var| args[var].write(
                      w, |w, var| write!(w, "v{}", var)
                    )
                  )
                }
              )
            )
          }
          Ok(()) as IoRes<()>
        } ;
        if self.lhs.is_empty() { Ok(()) } else {
          write!(writer, ") ")
        } ;
        self.rhs.write_smt2(
          writer, |w, prd, args| {
            cands[prd].write(
              w, |w, var| args[var].write(
                w, |w, var| write!(w, "v{}", var)
              )
            )
          }
        ) ;
        if self.lhs.is_empty() { Ok(()) } else {
          write!(writer, ")")
        } ;
        write!(writer, ")")
    }
  }
}




/// Stores the instance: the clauses, the factory and so on.
///
/// # NB
///
/// Clause indices can vary during instance building, because of the
/// simplifications that can remove clauses.
pub struct Instance {
  /// Term factory.
  factory: RwLock< HashConsign<RTerm> >,
  /// Constants constructed so far.
  consts: HConSet<RTerm>,
  /// Predicates.
  preds: PrdMap<PrdInfo>,
  /// Predicates for which a suitable term has been found.
  preds_term: PrdMap< Option<Term> >,
  /// Max arity of the predicates.
  pub max_pred_arity: Arity,
  /// Clauses.
  clauses: ClsMap<Clause>,
}
impl Instance {
  /// Instance constructor.
  pub fn mk(
    term_capa: usize, clauses_capa: usize, pred_capa: usize
  ) -> Instance {
    let mut instance = Instance {
      factory: RwLock::new(
        HashConsign::with_capacity(term_capa)
      ),
      consts: HConSet::with_capacity(103),
      preds: PrdMap::with_capacity(pred_capa),
      preds_term: PrdMap::with_capacity(pred_capa),
      max_pred_arity: 0.into(),
      clauses: ClsMap::with_capacity(clauses_capa),
    } ;
    // Create basic constants, adding to consts to have mining take them into account.
    let (wan,too) = (instance.one(), instance.zero()) ;
    instance.consts.insert(wan) ;
    instance.consts.insert(too) ;
    instance
  }


  /// Shrinks all collections.
  pub fn shrink_to_fit(& mut self) {
    self.consts.shrink_to_fit() ;
    self.preds.shrink() ;
    self.preds_term.shrink() ;
    self.clauses.shrink()
  }


  /// Returns the term we already know works for a predicate, if any.
  pub fn term_of(& self, pred: PrdIdx) -> Option<& Term> {
    self.preds_term[pred].as_ref()
  }

  /// Evaluates the term a predicate is forced to, if any.
  pub fn eval_term_of(
    & self, pred: PrdIdx, model: & VarMap<Val>
  ) -> Res< Option<bool> > {
    if let Some(term) = self.term_of(pred) {
      match term.bool_eval(model) {
        Ok(None) => bail!("partial model during predicate term evaluation"),
        res => res,
      }
    } else {
      Ok(None)
    }
  }

  /// Set of int constants **appearing in the predicates**. If more constants
  /// are created after the instance building step, they will not appear here.
  pub fn consts(& self) -> & HConSet<RTerm> {
    & self.consts
  }

  /// Range over the predicate indices.
  pub fn pred_indices(& self) -> PrdRange {
    PrdRange::zero_to( self.preds.len() )
  }

  /// Predicate accessor.
  pub fn preds(& self) -> & PrdMap<PrdInfo> {
    & self.preds
  }
  /// Clause accessor.
  pub fn clauses(& self) -> & ClsMap<Clause> {
    & self.clauses
  }

  /// Pushes a new predicate and returns its index.
  pub fn push_pred(
    & mut self, name: String, sig: VarMap<Typ>
  ) -> PrdIdx {
    self.max_pred_arity = ::std::cmp::max(
      self.max_pred_arity, sig.len().into()
    ) ;
    let idx = self.preds.next_index() ;
    self.preds.push( PrdInfo { name, idx, sig } ) ;
    self.preds_term.push(None) ;
    idx
  }

  /// Forces a predicate to be equal to something.
  pub fn force_pred(& mut self, pred: PrdIdx, term: Term) -> Res<()> {
    if let Some(t) = self.preds_term[pred].as_ref() {
      if t != & term {
        bail!(
          "[bug] trying to force predicate {} to {},\n\
          but it is already forced to be {}", self[pred], term, t
        )
      }
    }
    self.preds_term[pred] = Some(term) ;
    Ok(())
  }

  /// Forget a clause. **Does not preserve the order of the clauses.**
  pub fn forget_clause(& mut self, clause: ClsIdx) -> Clause {
    self.clauses.swap_remove(clause)
  }

  /// Pushes a new clause.
  pub fn push_clause(& mut self, clause: Clause) {
    self.clauses.push(clause)
  }


  /// Creates a variable.
  pub fn var(& self, v: VarIdx) -> Term {
    self.factory.mk( RTerm::Var(v) )
  }
  /// Creates a constant.
  pub fn int<I: Into<Int>>(& self, i: I) -> Term {
    self.factory.mk(
      RTerm::Int( i.into() )
    )
  }
  /// Creates the constant `0`.
  pub fn zero(& self) -> Term {
    self.int( Int::zero() )
  }
  /// Creates the constant `1`.
  pub fn one(& self) -> Term {
    self.int( Int::one() )
  }
  /// Creates a boolean.
  pub fn bool(& self, b: bool) -> Term {
    self.factory.mk( RTerm::Bool(b) )
  }
  /// Creates an operator application.
  pub fn op(& self, op: Op, args: Vec<Term>) -> Term {
    op.simplify(self, args)
  }

  /// Creates a less than or equal to.
  pub fn le(& self, lhs: Term, rhs: Term) -> Term {
    self.op(Op::Le, vec![lhs, rhs])
  }
  /// Creates a less than.
  pub fn lt(& self, lhs: Term, rhs: Term) -> Term {
    self.op(Op::Lt, vec![lhs, rhs])
  }
  /// Creates a greater than.
  pub fn gt(& self, lhs: Term, rhs: Term) -> Term {
    self.op(Op::Gt, vec![lhs, rhs])
  }
  /// Creates a greater than or equal to.
  pub fn ge(& self, lhs: Term, rhs: Term) -> Term {
    self.op(Op::Ge, vec![lhs, rhs])
  }

  /// Creates an equality.
  pub fn eq(& self, lhs: Term, rhs: Term) -> Term {
    self.op(Op::Eql, vec![lhs, rhs])
  }

  /// Turns a teacher counterexample into learning data.
  pub fn cexs_to_data(
    & self, data: & mut ::common::data::Data, cexs: ::teacher::Cexs
  ) -> Res<()> {

    for (clause, cex) in cexs.into_iter() {
      log_debug!{ "    working on clause {}...", clause }
      let clause = & self[clause] ;
      log_debug!{ "    getting antecedents..." }
      let mut antecedents = Vec::with_capacity( clause.lhs().len() ) ;
      log_debug!{ "    translating tterms..." }


      log_debug!{ "    working on lhs..." }
      for tterm in clause.lhs() {
        match * tterm {
          TTerm::P { pred, ref args } => {
            log_debug!{ "        pred: {} / {} ({})", pred, self.preds.len(), self.preds_term.len() }
            if self.preds_term[pred].is_none() {
              log_debug!{ "        -> is none" }
              let mut values = VarMap::with_capacity( args.len() ) ;
              for arg in args {
                values.push(
                  arg.eval(& cex).chain_err(
                    || "during argument evaluation to generate learning data"
                  ) ?
                )
              }
              antecedents.push(
                (pred, values)
              )
            } else {
              log_debug!{ "      -> is some" }
            }
          },
          _ => (),
        }
      }
      antecedents.shrink_to_fit() ;
      
      log_debug!{ "    working on rhs..." }
      let consequent = match * clause.rhs() {
        TTerm::P { pred, ref args } => {
          log_debug!{ "        pred: {} / {} ({})", pred, self.preds.len(), self.preds_term.len() }
          let mut values = VarMap::with_capacity( args.len() ) ;
          'pred_args: for arg in args {
            values.push(
              arg.eval(& cex).chain_err(
                || "during argument evaluation to generate learning data"
              ) ?
            )
          }
          Some( (pred, values) )
        },
        _ => None,
      } ;

      log_debug!{ "    antecedent: {:?}", antecedents }
      log_debug!{ "    consequent: {:?}", consequent }

      match ( antecedents.len(), consequent ) {
        (0, None) => bail!(
          "[unimplemented] clause with no predicate has a cex (unsafe)"
        ),
        (1, None) => {
          let (pred, args) = antecedents.pop().unwrap() ;
          data.stage_raw_neg(pred, args) ?
        },
        (0, Some( (pred, args) )) => data.stage_raw_pos(pred, args) ?,
        (_, consequent) => data.add_cstr(antecedents, consequent) ?,
      }
    }

    Ok(())
  }
}
impl ::std::ops::Index<ClsIdx> for Instance {
  type Output = Clause ;
  fn index(& self, index: ClsIdx) -> & Clause {
    & self.clauses[index]
  }
}
impl ::std::ops::Index<PrdIdx> for Instance {
  type Output = PrdInfo ;
  fn index(& self, index: PrdIdx) -> & PrdInfo {
    & self.preds[index]
  }
}













/// Operators.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Op {
  /// Addition.
  Add,
  /// Subtraction.
  Sub,
  /// Multiplication.
  Mul,
  /// Division.
  Div,
  /// Modulo.
  Mod,
  /// Greater than.
  Gt,
  /// Greater than or equal to.
  Ge,
  /// Less than or equal to.
  Le,
  /// Less than.
  Lt,
  /// Implication.
  Impl,
  /// Equal to.
  Eql,
  /// Negation.
  Not,
  /// Conjunction.
  And,
  /// Disjunction.
  Or,
}
impl Op {
  /// String representation.
  pub fn as_str(& self) -> & str {
    use instance::Op::* ;
    match * self {
      Add => "+", Sub => "-", Mul => "*", Div => "/", Mod => "mod",
      Gt => ">", Ge => ">=", Le => "<=", Lt => "<", Eql => "=",
      Not => "not", And => "and", Or => "or", Impl => "=>",
    }
  }

  /// Tries to simplify an operator application.
  ///
  /// Currently only simplifies nullary / unary applications of `And` and `Or`.
  ///
  /// ```
  /// let instance = & Instance::mk(10, 10, 10) ;
  /// let tru = instance.bool(true) ;
  /// let fls = instance.bool(false) ;
  /// let var_1 = instance.var( 7.into() ) ;
  /// let var_2 = instance.var( 2.into() ) ;
  ///
  /// assert_eq!( fls, Op::And.simplify(instance, vec![]) ) ;
  /// assert_eq!( tru, Op::Or.simplify(instance, vec![]) ) ;
  /// assert_eq!( var_2, Op::And.simplify(instance, vec![ var_2.clone() ]) ) ;
  /// assert_eq!( var_1, Op::Or.simplify(instance, vec![ var_1.clone() ]) ) ;
  /// let and = instance.op(Op::And, vec![ var_2.clone(), var_1.clone() ]) ;
  /// assert_eq!(
  ///   and, Op::And.simplify(instance, vec![ var_2.clone(), var_1.clone() ])
  /// ) ;
  /// let or = instance.op(Op::Or, vec![ var_2.clone(), and.clone() ]) ;
  /// assert_eq!(
  ///   or, Op::Or.simplify(instance, vec![ and.clone(), var_2.clone() ])
  /// ) ;
  /// ```
  pub fn simplify(
    self, instance: & Instance, mut args: Vec<Term>
  ) -> Term {
    let (op, args) = match self {
      Op::And => if args.is_empty() {
        return instance.bool(false)
      } else if args.len() == 1 {
        return args.pop().unwrap()
      } else {
        (self, args)
      },
      Op::Or => if args.is_empty() {
        return instance.bool(true)
      } else if args.len() == 1 {
        return args.pop().unwrap()
      } else {
        (self, args)
      },
      // Op::Gt => if args.len() != 2 {
      //   panic!( "[bug] operator `>` applied to {} operands", args.len() )
      // } else {
      //   if let Some( i ) = args[0].int_val() {
      //     // Checks whether it's zero before decreasing.
      //     if i.is_zero() {
      //       // Args is unchanged, term is equivalent to false anyway.
      //       (Op::Ge, args)
      //     } else {
      //       args[0] = instance.int(i - Int::one()) ;
      //       (Op::Ge, args)
      //     }
      //   } else if let Some( i ) = args[1].int_val() {
      //     args[1] = instance.int(i + Int::one()) ;
      //     (Op::Ge, args)
      //   } else {
      //     (self, args)
      //   }
      // },
      // Op::Lt => if args.len() != 2 {
      //   panic!( "[bug] operator `<` applied to {} operands", args.len() )
      // } else {
      //   if let Some( i ) = args[0].int_val() {
      //     args[0] = instance.int(i + Int::one()) ;
      //     (Op::Le, args)
      //   } else if let Some( i ) = args[1].int_val() {
      //     // Checks whether it's zero before decreasing.
      //     if i.is_zero() {
      //       // Args is unchanged, term is equivalent to false anyway.
      //       (Op::Le, args)
      //     } else {
      //       args[1] = instance.int(i - Int::one()) ;
      //       (Op::Le, args)
      //     }
      //   } else {
      //     (self, args)
      //   }
      // },
      _ => (self, args),
    } ;
    instance.factory.mk( RTerm::App { op, args } )
  }

  /// Operator parser.
  #[allow(unused_variables)]
  pub fn parse(
    bytes: & [u8]
  ) -> ::nom::IResult<& [u8], Self, Error> {
    fix_error!(
      bytes,
      Error,
      alt_complete!(
        map!(tag!("+"),   |_| Op::Add ) |
        map!(tag!("-"),   |_| Op::Sub ) |
        map!(tag!("*"),   |_| Op::Mul ) |
        map!(tag!("/"),   |_| Op::Div ) |
        map!(tag!("mod"), |_| Op::Mod ) |
        map!(tag!("<="),  |_| Op::Le  ) |
        map!(tag!("<"),   |_| Op::Lt  ) |
        map!(tag!(">="),  |_| Op::Ge  ) |
        map!(tag!(">"),   |_| Op::Gt  ) |
        map!(tag!("=>"),  |_| Op::Impl) |
        map!(tag!("="),   |_| Op::Eql ) |
        map!(tag!("not"), |_| Op::Not ) |
        map!(tag!("and"), |_| Op::And ) |
        map!(tag!("or"),  |_| Op::Or  )
      )
    )
  }


  /// Evaluation.
  pub fn eval(& self, mut args: Vec<Val>) -> Res<Val> {
    use instance::Op::* ;
    if args.is_empty() {
      bail!("evaluating operator on 0 elements")
    }
    match * self {
      Add => {
        let mut res ;
        for_first!{
          args.into_iter() => {
            |fst| res = try_val!(int fst),
            then |nxt| res = res + try_val!(int nxt),
            yild Ok( Val::I(res) )
          } else unreachable!()
        }
      },
      Sub => if args.len() == 1 {
        Ok(
          Val::I(
            - try_val!( int args.pop().unwrap() )
          )
        )
      } else {
        let mut res ;
        for_first!{
          args.into_iter() => {
            |fst| res = try_val!(int fst),
            then |nxt| res = res - try_val!(int nxt),
            yild Ok( Val::I(res) )
          } else unreachable!()
        }
      },
      Mul => {
        let mut res ;
        for_first!{
          args.into_iter() => {
            |fst| res = try_val!(int fst),
            then |nxt| res = res * try_val!(int nxt),
            yild Ok( Val::I(res) )
          } else unreachable!()
        }
      },
      Div => {
        let mut res ;
        for_first!{
          args.into_iter() => {
            |fst| res = try_val!(int fst),
            then |nxt| res = res / try_val!(int nxt),
            yild Ok( Val::I(res) )
          } else unreachable!()
        }
      },
      Mod => if args.len() != 2 {
        bail!(
          format!("evaluating `Div` with {} (!= 2) arguments", args.len())
        )
      } else {
        use num::Integer ;
        let b = try_val!( int args.pop().unwrap() ) ;
        let a = try_val!( int args.pop().unwrap() ) ;
        debug_assert!( args.is_empty() ) ;
        Ok(
          Val::I( a.mod_floor(& b) )
        )
      },

      // Bool operators.

      Gt => {
        let mut last ;
        for_first!{
          args.into_iter() => {
            |fst| last = try_val!(int fst),
            then |nxt| {
              let nxt = try_val!(int nxt) ;
              if last > nxt { last = nxt } else {
                return Ok( Val::B(false) )
              }
            },
            yild Ok( Val::B(true) )
          } else unreachable!()
        }
      },
      Ge => {
        let mut last ;
        for_first!{
          args.into_iter() => {
            |fst| last = try_val!(int fst),
            then |nxt| {
              let nxt = try_val!(int nxt) ;
              if last >= nxt { last = nxt } else {
                return Ok( Val::B(false) )
              }
            },
            yild Ok( Val::B(true) )
          } else unreachable!()
        }
      },
      Le => {
        let mut last ;
        for_first!{
          args.into_iter() => {
            |fst| last = try_val!(int fst),
            then |nxt| {
              let nxt = try_val!(int nxt) ;
              if last <= nxt { last = nxt } else {
                return Ok( Val::B(false) )
              }
            },
            yild Ok( Val::B(true) )
          } else unreachable!()
        }
      },
      Lt => {
        let mut last ;
        for_first!{
          args.into_iter() => {
            |fst| last = try_val!(int fst),
            then |nxt| {
              let nxt = try_val!(int nxt) ;
              if last < nxt { last = nxt } else {
                return Ok( Val::B(false) )
              }
            },
            yild Ok( Val::B(true) )
          } else unreachable!()
        }
      },
      Eql => {
        let mem ;
        for_first!{
          args.into_iter() => {
            |fst| mem = fst,
            then |nxt| {
              if mem != nxt {
                return Ok( Val::B(false) )
              }
            },
            yild Ok( Val::B(true) )
          } else unreachable!()
        }
      },
      Not => if args.len() != 1 {
        bail!(
          format!("evaluating `Not` with {} (!= 1) arguments", args.len())
        )
      } else {
        let b = try_val!( bool args.pop().unwrap() ) ;
        Ok( Val::B(! b) )
      },
      And => {
        let mut unknown = false ;
        for arg in args {
          match arg.to_bool() ? {
            Some(false) => return Ok( Val::B(false) ),
            None => unknown = true,
            _ => ()
          }
        }
        if unknown {
          Ok( Val::N )
        } else {
          Ok( Val::B(true) )
        }
      },
      Or => {
        let mut unknown = false ;
        for arg in args {
          match arg.to_bool() ? {
            Some(true) => return Ok( Val::B(true) ),
            None => unknown = true,
            _ => ()
          }
        }
        if unknown {
          Ok( Val::N )
        } else {
          Ok( Val::B(false) )
        }
      },
      Impl => if args.len() != 2 {
        bail!(
          format!("evaluating `Impl` with {} (!= 2) arguments", args.len())
        )
      } else {
        // Safe because of the check above.
        let rhs = args.pop().unwrap() ;
        let lhs = args.pop().unwrap() ;
        match ( lhs.to_bool() ?, rhs.to_bool() ? ) {
          (_, Some(true)) | (Some(false), _) => Ok( Val::B(true) ),
          (Some(lhs), Some(rhs)) => Ok( Val::B(rhs || ! lhs) ),
          _ => Ok(Val::N),
        }
      },
    }
  }
}
impl_fmt!{
  Op(self, fmt) {
    fmt.write_str( self.as_str() )
  }
}
















impl<'a> PebcakFmt<'a> for RTerm {
  type Info = & 'a VarMap<VarInfo> ;
  fn pebcak_err(& self) -> ErrorKind {
    "during term pebcak formatting".into()
  }
  fn pebcak_io_fmt<W: Write>(
    & self, w: & mut W, vars: & 'a VarMap<VarInfo>
  ) -> IoRes<()> {
    self.write(
      w, |w, var| w.write_all( vars[var].as_bytes() )
    )
  }
}

// impl<'a> PebcakFmt<'a> for TTerm {
//   type Info = (& 'a VarMap<VarInfo>, & 'a PrdMap<PrdInfo>) ;
//   fn pebcak_err(& self) -> ErrorKind {
//     "during top term pebcak formatting".into()
//   }
//   fn pebcak_io_fmt<W: Write>(
//     & self, w: & mut W,
//     (vars, prds): (& 'a VarMap<VarInfo>, & 'a PrdMap<PrdInfo>)
//   ) -> IoRes<()> {
//     self.write(
//       w,
//       |w, var| w.write_all( vars[var].as_bytes() ),
//       |w, prd| w.write_all( prds[prd].as_bytes() )
//     )
//   }
// }

impl<'a> PebcakFmt<'a> for Clause {
  type Info = & 'a PrdMap<PrdInfo> ;
  fn pebcak_err(& self) -> ErrorKind {
    "during clause pebcak formatting".into()
  }
  fn pebcak_io_fmt<W: Write>(
    & self, w: & mut W, prds: & 'a PrdMap<PrdInfo>
  ) -> IoRes<()> {
    self.write(
      w, |w, prd, args| {
        write!(w, "(") ? ;
        w.write_all( prds[prd].as_bytes() ) ? ;
        for arg in args {
          write!(w, " ") ? ;
          arg.write(w, |w, var| w.write_all( self.vars[var].as_bytes() )) ?
        }
        write!(w, ")")
      }
    )
  }
}

impl<'a> PebcakFmt<'a> for Instance {
  type Info = () ;
  fn pebcak_err(& self) -> ErrorKind {
    "during instance pebcak formatting".into()
  }
  fn pebcak_io_fmt<W: Write>(
    & self, w: & mut W, _: ()
  ) -> IoRes<()> {
    for pred in & self.preds {
      write!(
        w, "({}\n  {} (", ::common::consts::keywords::prd_dec, pred.name
      ) ? ;
      for typ in & pred.sig {
        write!(w, " {}", typ) ?
      }
      write!(w, " )\n)\n") ?
    }
    for clause in & self.clauses {
      write!(w, "\n") ? ;
      clause.pebcak_io_fmt(w, & self.preds) ?
    }
    write!(w, "\n")
  }
}








#[test]
fn simplify() {
  let instance = & Instance::mk(10, 10, 10) ;
  let tru = instance.bool(true) ;
  let fls = instance.bool(false) ;
  let var_1 = instance.var( 7.into() ) ;
  let var_2 = instance.var( 2.into() ) ;

  assert_eq!( fls, Op::And.simplify(instance, vec![]) ) ;
  assert_eq!( tru, Op::Or.simplify(instance, vec![]) ) ;
  assert_eq!( var_2, Op::And.simplify(instance, vec![ var_2.clone() ]) ) ;
  assert_eq!( var_1, Op::Or.simplify(instance, vec![ var_1.clone() ]) ) ;
  let and = instance.op(Op::And, vec![ var_2.clone(), var_1.clone() ]) ;
  assert_eq!(
    and, Op::And.simplify(instance, vec![ var_2.clone(), var_1.clone() ])
  ) ;
  let or = instance.op(Op::Or, vec![ var_2.clone(), and.clone() ]) ;
  assert_eq!(
    or, Op::Or.simplify(instance, vec![ var_2.clone(), and.clone() ])
  ) ;
}





#[cfg(test)]
mod evaluation {
  // use common::* ;
  use instance::* ;

  #[cfg(test)]
  macro_rules! model {
    ( $($values:expr),* ) => (
      $crate::common::VarMap::of(
        vec![ $( $values.into() ),* ]
      )
    ) ;
  }

  #[cfg(test)]
  macro_rules! assert_eval {
    ( int $model:expr => $expr:expr, $value:expr ) => ({
      let res = $expr.eval(& $model).unwrap().to_int().unwrap().unwrap() ;
      println!(
        "{} evaluated with {} is {}, expecting {}", $expr, $model, res, $value
      ) ;
      assert_eq!( res, $value.into() )
    }) ;
    ( bool $model:expr => $expr:expr ) => ({
      let res = $expr.eval(& $model).unwrap().to_bool().unwrap().unwrap() ;
      println!(
        "{} evaluated with {} is {}, expecting true", $expr, $model, res
      ) ;
      assert!( res )
    }) ;
    ( bool not $model:expr => $expr:expr ) => ({
      let res = $expr.eval(& $model).unwrap().to_bool().unwrap().unwrap() ;
      println!(
        "{} evaluated with {} is {}, expecting false", $expr, $model, res
      ) ;
      assert!( ! res )
    }) ;
  }

  /// Just creates an instance.
  fn instance() -> Instance {
    Instance::mk(100, 100, 100)
  }

  #[test]
  fn cst_add() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(3) ;
    let sum = instance.op( Op::Add, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      int model => sum, 10
    )
  }

  #[test]
  fn cst_sub_1() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(3) ;
    let sub = instance.op( Op::Sub, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      int model => sub, 4
    )
  }

  #[test]
  fn cst_sub_2() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let sub = instance.op( Op::Sub, vec![ c_1 ] ) ;
    let model = model!() ;
    assert_eval!(
      int model => sub, (-7)
    )
  }

  #[test]
  fn cst_mul() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(3) ;
    let mul = instance.op( Op::Mul, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      int model => mul, 21
    )
  }

  #[test]
  fn cst_div() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(3) ;
    let div = instance.op( Op::Div, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      int model => div, 2
    )
  }

  #[test]
  fn cst_mod() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(3) ;
    let m0d = instance.op( Op::Mod, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      int model => m0d, 1
    )
  }

  #[test]
  fn cst_gt_1() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(3) ;
    let gt = instance.op( Op::Gt, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => gt
    )
  }

  #[test]
  fn cst_gt_2() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(7) ;
    let gt = instance.op( Op::Gt, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => gt
    )
  }

  #[test]
  fn cst_ge_1() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(3) ;
    let ge = instance.op( Op::Ge, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => ge
    )
  }

  #[test]
  fn cst_ge_2() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(7) ;
    let ge = instance.op( Op::Ge, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => ge
    )
  }

  #[test]
  fn cst_le_1() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(3) ;
    let le = instance.op( Op::Le, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => le
    )
  }

  #[test]
  fn cst_le_2() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(7) ;
    let le = instance.op( Op::Le, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => le
    )
  }

  #[test]
  fn cst_lt_1() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(3) ;
    let lt = instance.op( Op::Lt, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => lt
    )
  }

  #[test]
  fn cst_lt_2() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(7) ;
    let lt = instance.op( Op::Lt, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => lt
    )
  }

  #[test]
  fn cst_eq_1() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(7) ;
    let eq = instance.op( Op::Eql, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => eq
    )
  }

  #[test]
  fn cst_eq_2() {
    let instance = instance() ;
    let c_1 = instance.int(7) ;
    let c_2 = instance.int(3) ;
    let eq = instance.op( Op::Eql, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => eq
    )
  }

  #[test]
  fn cst_eq_3() {
    let instance = instance() ;
    let c_1 = instance.bool(true) ;
    let c_2 = instance.bool(true) ;
    let eq = instance.op( Op::Eql, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => eq
    )
  }

  #[test]
  fn cst_eq_4() {
    let instance = instance() ;
    let c_1 = instance.bool(false) ;
    let c_2 = instance.bool(true) ;
    let eq = instance.op( Op::Eql, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => eq
    )
  }

  #[test]
  fn cst_impl_1() {
    let instance = instance() ;
    let c_1 = instance.bool(false) ;
    let c_2 = instance.bool(false) ;
    let imp = instance.op( Op::Impl, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => imp
    )
  }

  #[test]
  fn cst_impl_2() {
    let instance = instance() ;
    let c_1 = instance.bool(true) ;
    let c_2 = instance.bool(false) ;
    let imp = instance.op( Op::Impl, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => imp
    )
  }

  #[test]
  fn cst_impl_3() {
    let instance = instance() ;
    let c_1 = instance.bool(false) ;
    let c_2 = instance.bool(true) ;
    let imp = instance.op( Op::Impl, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => imp
    )
  }

  #[test]
  fn cst_impl_4() {
    let instance = instance() ;
    let c_1 = instance.bool(true) ;
    let c_2 = instance.bool(true) ;
    let imp = instance.op( Op::Impl, vec![ c_1, c_2 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => imp
    )
  }

  #[test]
  fn cst_not_1() {
    let instance = instance() ;
    let c_1 = instance.bool(false) ;
    let not = instance.op( Op::Not, vec![ c_1 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => not
    )
  }

  #[test]
  fn cst_not_2() {
    let instance = instance() ;
    let c_1 = instance.bool(true) ;
    let not = instance.op( Op::Not, vec![ c_1 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => not
    )
  }

  #[test]
  fn cst_and_1() {
    let instance = instance() ;
    let c_1 = instance.bool(true) ;
    let c_2 = instance.bool(true) ;
    let c_3 = instance.bool(true) ;
    let c_4 = instance.bool(true) ;
    let and = instance.op( Op::And, vec![ c_1, c_2, c_3, c_4 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => and
    )
  }

  #[test]
  fn cst_and_2() {
    let instance = instance() ;
    let c_1 = instance.bool(true) ;
    let c_2 = instance.bool(true) ;
    let c_3 = instance.bool(false) ;
    let c_4 = instance.bool(true) ;
    let and = instance.op( Op::And, vec![ c_1, c_2, c_3, c_4 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => and
    )
  }

  #[test]
  fn cst_and_3() {
    let instance = instance() ;
    let c_1 = instance.bool(false) ;
    let c_2 = instance.bool(true) ;
    let c_3 = instance.bool(true) ;
    let c_4 = instance.bool(true) ;
    let and = instance.op( Op::And, vec![ c_1, c_2, c_3, c_4 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => and
    )
  }

  #[test]
  fn cst_and_4() {
    let instance = instance() ;
    let c_1 = instance.bool(true) ;
    let c_2 = instance.bool(false) ;
    let c_3 = instance.bool(false) ;
    let c_4 = instance.bool(true) ;
    let and = instance.op( Op::And, vec![ c_1, c_2, c_3, c_4 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => and
    )
  }

  #[test]
  fn cst_or_1() {
    let instance = instance() ;
    let c_1 = instance.bool(true) ;
    let c_2 = instance.bool(true) ;
    let c_3 = instance.bool(true) ;
    let c_4 = instance.bool(true) ;
    let or = instance.op( Op::Or, vec![ c_1, c_2, c_3, c_4 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => or
    )
  }

  #[test]
  fn cst_or_2() {
    let instance = instance() ;
    let c_1 = instance.bool(true) ;
    let c_2 = instance.bool(true) ;
    let c_3 = instance.bool(false) ;
    let c_4 = instance.bool(true) ;
    let or = instance.op( Op::Or, vec![ c_1, c_2, c_3, c_4 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => or
    )
  }

  #[test]
  fn cst_or_3() {
    let instance = instance() ;
    let c_1 = instance.bool(false) ;
    let c_2 = instance.bool(true) ;
    let c_3 = instance.bool(true) ;
    let c_4 = instance.bool(true) ;
    let or = instance.op( Op::Or, vec![ c_1, c_2, c_3, c_4 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => or
    )
  }

  #[test]
  fn cst_or_4() {
    let instance = instance() ;
    let c_1 = instance.bool(true) ;
    let c_2 = instance.bool(false) ;
    let c_3 = instance.bool(false) ;
    let c_4 = instance.bool(true) ;
    let or = instance.op( Op::Or, vec![ c_1, c_2, c_3, c_4 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool model => or
    )
  }

  #[test]
  fn cst_or_5() {
    let instance = instance() ;
    let c_1 = instance.bool(false) ;
    let c_2 = instance.bool(false) ;
    let c_3 = instance.bool(false) ;
    let c_4 = instance.bool(false) ;
    let or = instance.op( Op::Or, vec![ c_1, c_2, c_3, c_4 ] ) ;
    let model = model!() ;
    assert_eval!(
      bool not model => or
    )
  }

  #[test]
  fn models() {
    let instance = instance() ;
    let v_1 = instance.var( 0.into() ) ;
    let v_2 = instance.var( 1.into() ) ;
    let v_3 = instance.var( 2.into() ) ;


    let model_1 = model!( true, 2, 3 ) ;
    let model_2 = model!( true, 7, 0 ) ;

    // (7 - v_2) + (v_2 * 2) + (- v_3)
    let lhs = instance.op(
      Op::Add, vec![
        instance.op( Op::Sub, vec![ instance.int(7), v_2.clone() ] ),
        instance.op( Op::Mul, vec![ v_2.clone(), instance.int(2) ] ),
        instance.op( Op::Sub, vec![ v_3.clone() ] ),
      ]
    ) ;
    assert_eval!(int model_1 => lhs, 6) ;
    assert_eval!(int model_2 => lhs, 14) ;

    // v_3 * 3
    let rhs = instance.op(
      Op::Mul, vec![ v_3.clone(), instance.int(3) ]
    ) ;
    assert_eval!(int model_1 => rhs, 9) ;
    assert_eval!(int model_2 => rhs, 0) ;

    // 7 + v_2 + (- v_3) > v_3 * 3
    let gt = instance.op(
      Op::Gt, vec![ lhs.clone(), rhs.clone() ]
    ) ;
    assert_eval!(bool not model_1 => gt) ;
    assert_eval!(bool     model_2 => gt) ;

    // v_1 && (7 + v_2 + (- v_3) > v_3 * 3)
    let and = instance.op(
      Op::And, vec![ v_1.clone(), gt.clone() ]
    ) ;
    assert_eval!(bool not model_1 => and) ;
    assert_eval!(bool     model_2 => and) ;

    ()
  }
}