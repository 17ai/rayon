use super::*;

#[test]
pub fn execute() {
    let a: Vec<i32> = (0..1024).collect();
    let mut b = vec![];
    a.into_par_iter()
     .map(|&i| i + 1)
     .collect_into(&mut b);
    let c: Vec<i32> = (0..1024).map(|i| i+1).collect();
    assert_eq!(b, c);
}

#[test]
pub fn map_reduce() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.into_par_iter()
              .map(|&i| i + 1)
              .sum();
    let r2 = a.iter()
              .map(|&i| i + 1)
              .fold(0, |a,b| a+b);
    assert_eq!(r1, r2);
}

#[test]
pub fn map_reduce_with() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.into_par_iter()
              .map(|&i| i + 1)
              .reduce_with(|i, j| i + j);
    let r2 = a.iter()
              .map(|&i| i + 1)
              .fold(0, |a,b| a+b);
    assert_eq!(r1.unwrap(), r2);
}

#[test]
pub fn map_reduce_weighted() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.into_par_iter()
              .map(|&i| i + 1)
              .weight(2.0)
              .reduce_with(|i, j| i + j);
    let r2 = a.iter()
              .map(|&i| i + 1)
              .fold(0, |a,b| a+b);
    assert_eq!(r1.unwrap(), r2);
}
