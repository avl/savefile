use assert_roundtrip;
use savefile::{get_schema, Removed, WithSchema, WithSchemaContext};

#[derive(Savefile, Debug, PartialEq)]
enum Tree {
    Leaf,
    Node(Box<Tree>,Box<Tree>)
}

#[test]
pub fn test_cyclic() {
    let example = Tree::Node(Box::new(Tree::Leaf), Box::new(Tree::Leaf));
    assert_roundtrip(example);

    let example = Tree::Node(Box::new(Tree::Node(Box::new(Tree::Leaf),Box::new(Tree::Leaf))), Box::new(Tree::Leaf));
    assert_roundtrip(example);
}


#[derive(Savefile, Debug, PartialEq)]
struct TreeNode {
    tree: Box<Tree2>
}

#[derive(Savefile, Debug, PartialEq)]
enum Tree2 {
    Leaf(String),
    Node(TreeNode)
}
#[test]
pub fn test_cyclic2() {
    let example = Tree2::Node(TreeNode{tree: Box::new(Tree2::Leaf("hej".into()))});
    assert_roundtrip(example);
}

#[derive(Savefile, Debug, PartialEq)]
struct Version1LevelC(Box<Version1LevelB>);

#[derive(Savefile, Debug, PartialEq)]
struct Version1LevelB(Box<Version1LevelC>);

#[derive(Savefile, Debug, PartialEq)]
struct Version1LevelA(Option<Box<Version1LevelB>>);

#[derive(Savefile, Debug, PartialEq)]
struct Version1Base(Option<Box<Version1LevelA>>);


#[derive(Savefile, Debug, PartialEq)]
struct Version2LevelC(Box<Version2LevelA>);

#[derive(Savefile, Debug, PartialEq)]
struct Version2LevelB(Box<Version2LevelC>);

#[derive(Savefile, Debug, PartialEq)]
struct Version2LevelA(Option<Box<Version2LevelB>>);
#[derive(Savefile, Debug, PartialEq)]
struct Version2Base(Option<Box<Version2LevelA>>);


#[test]
#[should_panic(expected = "Saved schema differs from in-memory schema for version 0. Error: At location [./Version1Base/0/?/Version1LevelA/0/?/Version1LevelB/0/Version1LevelC/0]: Application protocol uses recursion up 3 levels, but foreign format uses 2")]
fn cycles_vertest1() {
    use assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        Version1Base(None),
        0,
        Version2Base(None),
        1,
    );
}