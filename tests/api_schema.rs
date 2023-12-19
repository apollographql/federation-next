use apollo_compiler::ast;
use apollo_compiler::ast::Name;
use apollo_compiler::schema;
use apollo_compiler::validation::Valid;
use apollo_compiler::Schema;
use apollo_federation::Supergraph;
use std::collections::HashSet;

fn api_schema(input: &str) -> Valid<Schema> {
    let graph = Supergraph::new(input).unwrap();
    graph.to_api_schema().unwrap()
}

const SUPERGRAPH_SDL: &str = r#"
schema
    @core(feature: "https://specs.apollo.dev/core/v0.2"),
    @core(feature: "https://specs.apollo.dev/join/v0.1", for: EXECUTION)
{
    query: Query
    mutation: Mutation
}

directive @core(feature: String!, as: String, for: core__Purpose) repeatable on SCHEMA

directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet) on FIELD_DEFINITION

directive @join__type(graph: join__Graph!, key: join__FieldSet) repeatable on OBJECT | INTERFACE

directive @join__owner(graph: join__Graph!) on OBJECT | INTERFACE

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @stream on FIELD

directive @fragmentDirective on INLINE_FRAGMENT

directive @transform(from: String!) on FIELD

type Account {
    type: String
}

union AccountType = PasswordAccount | SMSAccount

type Amazon {
    referrer: String
}

union Body = Image | Text

type Book implements Product
    @join__owner(graph: BOOKS)
    @join__type(graph: BOOKS, key: "isbn")
    @join__type(graph: INVENTORY, key: "isbn")
    @join__type(graph: PRODUCT, key: "isbn")
    @join__type(graph: REVIEWS, key: "isbn")
{
    isbn: String! @join__field(graph: BOOKS)
    title: String @join__field(graph: BOOKS)
    year: Int @join__field(graph: BOOKS)
    similarBooks: [Book]! @join__field(graph: BOOKS)
    metadata: [MetadataOrError] @join__field(graph: BOOKS)
    inStock: Boolean @join__field(graph: INVENTORY)
    isCheckedOut: Boolean @join__field(graph: INVENTORY)
    upc: String! @join__field(graph: PRODUCT)
    sku: String! @join__field(graph: PRODUCT)
    name(delimeter: String = " "): String @join__field(graph: PRODUCT, requires: "title year")
    price: String @join__field(graph: PRODUCT)
    details: ProductDetailsBook @join__field(graph: PRODUCT)
    reviews: [Review] @join__field(graph: REVIEWS)
    relatedReviews: [Review!]! @join__field(graph: REVIEWS, requires: "similarBooks { isbn }")
}

union Brand = Ikea | Amazon

type Car implements Vehicle
    @join__owner(graph: PRODUCT)
    @join__type(graph: PRODUCT, key: "id")
    @join__type(graph: REVIEWS, key: "id")
{
    id: String! @join__field(graph: PRODUCT)
    description: String @join__field(graph: PRODUCT)
    price: String @join__field(graph: PRODUCT)
    retailPrice: String @join__field(graph: REVIEWS, requires: "price")
    thing: Thing
}

enum core__Purpose {
  EXECUTION
  SECURITY
}

type Error {
    code: Int
    message: String
}

type Furniture implements Product
    @join__owner(graph: PRODUCT)
    @join__type(graph: PRODUCT, key: "upc")
    @join__type(graph: PRODUCT, key: "sku")
    @join__type(graph: INVENTORY, key: "sku")
    @join__type(graph: REVIEWS, key: "upc")
{
    upc: String! @join__field(graph: PRODUCT)
    sku: String! @join__field(graph: PRODUCT)
    name: String @join__field(graph: PRODUCT)
    price: String @join__field(graph: PRODUCT)
    brand: Brand @join__field(graph: PRODUCT)
    metadata: [MetadataOrError] @join__field(graph: PRODUCT)
    details: ProductDetailsFurniture @join__field(graph: PRODUCT)
    inStock: Boolean @join__field(graph: INVENTORY)
    isHeavy: Boolean @join__field(graph: INVENTORY)
    reviews: [Review] @join__field(graph: REVIEWS)
}

type Ikea {
    asile: Int
}

type Image {
    name: String!
    attributes: ImageAttributes!
}

type ImageAttributes {
    url: String!
}

scalar join__FieldSet

enum join__Graph {
    ACCOUNTS @join__graph(name: "accounts" url: "undefined")
    BOOKS @join__graph(name: "books" url: "undefined")
    DOCUMENTS @join__graph(name: "documents" url: "undefined")
    INVENTORY @join__graph(name: "inventory" url: "undefined")
    PRODUCT @join__graph(name: "product" url: "undefined")
    REVIEWS @join__graph(name: "reviews" url: "undefined")
}

type KeyValue {
    key: String!
    value: String!
}

type Library
    @join__owner(graph: BOOKS)
    @join__type(graph: BOOKS, key: "id")
    @join__type(graph: ACCOUNTS, key: "id")
{
    id: ID! @join__field(graph: BOOKS)
    name: String @join__field(graph: BOOKS)
    userAccount(id: ID! = 1): User @join__field(graph: ACCOUNTS, requires: "name")
}

union MetadataOrError = KeyValue | Error

type Mutation {
    login(username: String!, password: String!): User @join__field(graph: ACCOUNTS)
    reviewProduct(upc: String!, body: String!): Product @join__field(graph: REVIEWS)
    updateReview(review: UpdateReviewInput!): Review @join__field(graph: REVIEWS)
    deleteReview(id: ID!): Boolean @join__field(graph: REVIEWS)
}

type Name {
    first: String
    last: String
}

type PasswordAccount
    @join__owner(graph: ACCOUNTS)
    @join__type(graph: ACCOUNTS, key: "email")
{
    email: String! @join__field(graph: ACCOUNTS)
}

interface Product {
    upc: String!
    sku: String!
    name: String
    price: String
    details: ProductDetails
    inStock: Boolean
    reviews: [Review]
}

interface ProductDetails {
    country: String
}

type ProductDetailsBook implements ProductDetails {
    country: String
    pages: Int
}

type ProductDetailsFurniture implements ProductDetails {
    country: String
    color: String
}

type Query {
    user(id: ID!): User @join__field(graph: ACCOUNTS)
    me: User @join__field(graph: ACCOUNTS)
    book(isbn: String!): Book @join__field(graph: BOOKS)
    books: [Book] @join__field(graph: BOOKS)
    library(id: ID!): Library @join__field(graph: BOOKS)
    body: Body! @join__field(graph: DOCUMENTS)
    product(upc: String!): Product @join__field(graph: PRODUCT)
    vehicle(id: String!): Vehicle @join__field(graph: PRODUCT)
    topProducts(first: Int = 5): [Product] @join__field(graph: PRODUCT)
    topCars(first: Int = 5): [Car] @join__field(graph: PRODUCT)
    topReviews(first: Int = 5): [Review] @join__field(graph: REVIEWS)
}

type Review
    @join__owner(graph: REVIEWS)
    @join__type(graph: REVIEWS, key: "id")
{
    id: ID! @join__field(graph: REVIEWS)
    body(format: Boolean = false): String @join__field(graph: REVIEWS)
    author: User @join__field(graph: REVIEWS, provides: "username")
    product: Product @join__field(graph: REVIEWS)
    metadata: [MetadataOrError] @join__field(graph: REVIEWS)
}

type SMSAccount
    @join__owner(graph: ACCOUNTS)
    @join__type(graph: ACCOUNTS, key: "number")
{
    number: String @join__field(graph: ACCOUNTS)
}

type Text {
    name: String!
    attributes: TextAttributes!
}

type TextAttributes {
    bold: Boolean
    text: String
}

union Thing = Car | Ikea

input UpdateReviewInput {
    id: ID!
    body: String
}

type User
    @join__owner(graph: ACCOUNTS)
    @join__type(graph: ACCOUNTS, key: "id")
    @join__type(graph: ACCOUNTS, key: "username name { first last }")
    @join__type(graph: INVENTORY, key: "id")
    @join__type(graph: PRODUCT, key: "id")
    @join__type(graph: REVIEWS, key: "id")
{
    id: ID! @join__field(graph: ACCOUNTS)
    name: Name @join__field(graph: ACCOUNTS)
    username: String @join__field(graph: ACCOUNTS)
    birthDate(locale: String): String @join__field(graph: ACCOUNTS)
    account: Account @join__field(graph: ACCOUNTS)
    accountType: AccountType @join__field(graph: ACCOUNTS)
    metadata: [UserMetadata] @join__field(graph: ACCOUNTS)
    goodDescription: Boolean @join__field(graph: INVENTORY, requires: "metadata { description }")
    vehicle: Vehicle @join__field(graph: PRODUCT)
    thing: Thing @join__field(graph: PRODUCT)
    reviews: [Review] @join__field(graph: REVIEWS)
    numberOfReviews: Int! @join__field(graph: REVIEWS)
    goodAddress: Boolean @join__field(graph: REVIEWS, requires: "metadata { address }")
}

type UserMetadata {
    name: String
    address: String
    description: String
}

type Van implements Vehicle
    @join__owner(graph: PRODUCT)
    @join__type(graph: PRODUCT, key: "id")
    @join__type(graph: REVIEWS, key: "id")
{
    id: String! @join__field(graph: PRODUCT)
    description: String @join__field(graph: PRODUCT)
    price: String @join__field(graph: PRODUCT)
    retailPrice: String @join__field(graph: REVIEWS, requires: "price")
}

interface Vehicle {
    id: String!
    description: String
    price: String
    retailPrice: String
}
"#;

// trait DirectiveNames {
//     fn collect_directive_names(&self, output: &mut HashSet<Name>) -> ();
// }

// impl DirectiveNames for Schema {
//     fn collect_directive_names(&self, output: &mut HashSet<Name>) {
//         for dir in &self.schema_definition.directives {
//             dir.collect_directive_names(output);
//         }

//         for dir_def in &self.directive_definitions {
//             dir_def.collect_directive_names(output);
//         }
//     }
// }
// impl DirectiveNames for ast::Directive {
//     fn collect_directive_names(&self, output: &mut HashSet<Name>) {
//         output.insert(self.name.clone());
//     }
// }
// impl DirectiveNames for schema::DirectiveDefinition {
//     fn collect_directive_names(&self, output: &mut HashSet<Name>) {
//         for dir in self.directives() {
//             dir.collect_directive_names(output);
//         }
//     }
// }

// fn directive_names(s: &Schema) -> HashSet<Name> {
//     let mut names = HashSet::new();
// }

#[test]
fn removed_core_directives() {
    let s = api_schema(SUPERGRAPH_SDL);
}
