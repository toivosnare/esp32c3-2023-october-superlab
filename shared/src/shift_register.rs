#[derive(PartialEq)]
pub struct ShiftRegister;

impl ShiftRegister {
    pub fn insert(&self, val: u64) {
        todo!()
    }
    pub fn avg(&self) -> u64 {
        todo!()
    }

    /// Don't use this. It's for testing :-)
    fn as_array(&self) -> &[Option<u64>] {
        todo!()
    }
}

#[test]
fn insert_works() {
    let mut sr = ShiftRegister;

    sr.insert(1);
    assert_eq!(sr.as_array(), &[Some(1), None, None]);

    sr.insert(2);
    assert_eq!(sr.as_array(), &[Some(2), Some(1), None]);

    sr.insert(3);
    assert_eq!(sr.as_array(), &[Some(3), Some(2), Some(1)]);

    sr.insert(4);
    assert_eq!(sr.as_array(), &[Some(4), Some(3), Some(2)]);
}

fn avg_works() {
    let sr = ShiftRegister;

    sr.insert(1);
    sr.insert(2);
    sr.insert(3);

    assert_eq!(sr.avg(), 1);

    sr.insert(4);

    assert_eq!(sr.avg(), 1);
}
