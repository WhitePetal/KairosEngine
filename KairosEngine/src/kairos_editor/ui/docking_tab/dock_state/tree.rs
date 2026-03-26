pub mod node;


#[derive(Clone, Debug)]
pub struct TabIndex(pub usize);

impl From<usize> for TabIndex {
    #[inline(always)]
    fn from(index: usize) -> Self {
        TabIndex(index)
    }
}



pub struct Tree {

}