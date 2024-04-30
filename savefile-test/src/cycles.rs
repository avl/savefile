use assert_roundtrip;
use savefile::Removed;

#[derive(Savefile, Debug, PartialEq)]
enum Tree {
    Leaf,
    Node(Box<Tree>, Box<Tree>),
}

#[test]
pub fn test_cyclic() {
    let example = Tree::Node(Box::new(Tree::Leaf), Box::new(Tree::Leaf));
    assert_roundtrip(example);

    let example = Tree::Node(
        Box::new(Tree::Node(Box::new(Tree::Leaf), Box::new(Tree::Leaf))),
        Box::new(Tree::Leaf),
    );
    assert_roundtrip(example);
}

#[derive(Savefile, Debug, PartialEq)]
struct TreeNode {
    tree: Box<Tree2>,
}

#[derive(Savefile, Debug, PartialEq)]
enum Tree2 {
    Leaf(String),
    Node(TreeNode),
}
#[test]
pub fn test_cyclic2() {
    let example = Tree2::Node(TreeNode {
        tree: Box::new(Tree2::Leaf("hej".into())),
    });
    assert_roundtrip(example);
}
#[derive(Savefile, Debug, PartialEq)]
enum Version1LevelD {
    Leaf,
    Node(Box<Version1LevelA>),
}

#[derive(Savefile, Debug, PartialEq)]
enum Version1LevelC {
    Leaf,
    Node(Box<Version1LevelD>),
}

#[derive(Savefile, Debug, PartialEq)]
enum Version1LevelB {
    Leaf(Box<Version1LevelC>),
    Node(Box<Version1LevelC>),
}

#[derive(Savefile, Debug, PartialEq)]
enum Version1LevelA {
    Leaf,
    Node(Box<Version1LevelB>),
}

#[derive(Savefile, Debug, PartialEq)]
enum Version2LevelC {
    Leaf,
    Node(Box<Version2LevelA>),
}

#[derive(Savefile, Debug, PartialEq)]
enum Version2LevelB {
    Leaf(Box<Version2LevelC>),
    Node(Box<Version2LevelC>),
}

#[derive(Savefile, Debug, PartialEq)]
enum Version2LevelA {
    Leaf,
    Node(Box<Version2LevelB>),
}

#[test]
#[should_panic(
    expected = "Saved schema differs from in-memory schema for version 0. Error: At location [.Version1LevelA/Node/0Version1LevelB/Leaf/0Version1LevelC/Node/0Version1LevelD/Node/0]: In memory schema: <recursion 3>, file schema: enum"
)]
fn cycles_vertest1() {
    use assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(Version1LevelA::Leaf, 0, Version2LevelA::Leaf, 1);
}
