/*
#[graph]
mod make_app {
  pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> App {
    App { name, comp1, comp2 }
  }

  fn comp1() -> Comp1 {
    Comp1::new()
  }

  fn comp2() -> Comp2 {
    Comp2::new()
  }
}

mod make_app {
  pub fn run(name: String) -> App {
    let comp1 = comp1();
    let comp2 = comp2();
    return run_impl(
      name.into(),
      comp1.into(),
      comp2.into(),
    );
  }

  fn run_impl(name: String, comp1: Comp1, comp2: Comp2) -> App {
    App { name, comp1, comp2 }
  }

  fn comp1() -> Box<Comp1> {
    Comp1::new()
  }

  fn comp2() -> Comp2 {
    Comp2::new()
  }
}



#[graph]
mod make_app {
  pub fn run(name: String, c1: C1, c2: C2) -> App {
    App { name, c1, c2 }
  }

  pub async fn c1(c3: C3) -> C1 {
    C1::new(c3)
  }

  async fn c2(c3: C3) -> C2 {
    C2::new(c3)
  }

  async fn c3() -> C3 {
    C3::new()
  }
}

mod make_app {
  pub fn run(name: String) -> App {
    let c3 = c3();
    let c3 = c3.await; // Converts Future<C3> -> C3
    let c1 = c1_impl(c3.clone()); // c3.clone() because it is referenced later
    let c2 = c2(c3);
    let (c1, c2) = tokio::join!(c1, c2); // Converts multiple parameters Future<T> -> T
    return run_impl(
      name.into(),
      c1.into(),
      c2.into(),
    );
  }

  // Preserve the original impl
  fn run_impl(name: String, c1: C1, 2: C2) -> App {
    App { name, c1, c2 }
  }

  pub async fn c1() -> C1 {
    let c3 = c3();
    C1::new(c3)
  }

  // Preserve the original impl
  async fn c1_impl(c3: C3) -> C1 {
    C1::new(c3)
  }

  // Impl and derived are the same because there is no need to generate pub c2()
  async fn c2(c3: C3) -> C2 {
    C2::new(c3)
  }

  // Impl and derived are the same because there is no need to generate pub c3()
  async fn c3() -> C3 {
    C3::new()
  }
}
*/
