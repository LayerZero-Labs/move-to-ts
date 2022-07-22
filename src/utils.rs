use itertools::Itertools;
use move_compiler::expansion::ast::ModuleIdent;
use std::fmt;

pub fn generate_package_json(package_name: String) -> (String, String) {
    let content = format!(
        r###"
{{
  "name": "{}",
  "version": "0.0.1",
  "scripts": {{
    "build": "rm -rf dist; tsc -p tsconfig.json",
    "test": "jest"
  }},
  "main": "dist/index.js",
  "typings": "dist/index.d.ts",
  "files": [ "src", "dist" ],
  "devDependencies": {{
    "@types/jest": "^27.4.1",
    "@types/node": "^17.0.31",
    "@typescript-eslint/eslint-plugin": "^5.22.0",
    "@typescript-eslint/parser": "^5.22.0",
    "eslint": "^8.15.0",
    "eslint-config-prettier": "^8.5.0",
    "eslint-plugin-prettier": "^4.0.0",
    "jest": "^27.5.1",
    "prettier": "^2.6.2",
    "ts-jest": "^27.1.4",
    "typescript": "^4.6.4"
  }},
  "dependencies": {{
    "aptos": "^1.2.0",
    "big-integer": "^1.6.51",
    "@manahippo/move-to-ts": "^0.0.49"
  }}
}}
"###,
        package_name
    );
    ("package.json".to_string(), content)
}

pub fn generate_ts_config() -> (String, String) {
    let content = r###"
{
  "compilerOptions": {
    "target": "es2016",
    "module": "commonjs",
    "rootDir": "./src",
    "moduleResolution": "node",
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true,
    "outDir": "./dist",
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "strict": true,
    "skipLibCheck": true
  }
}
"###;
    ("tsconfig.json".to_string(), content.to_string())
}

pub fn generate_jest_config() -> (String, String) {
    let content = r###"
module.exports = {
  preset: "ts-jest",
  testEnvironment: "node",
  testPathIgnorePatterns: ["dist/*"],
};
"###;
    ("jest.config.js".to_string(), content.to_string())
}

/*
1. Replace typescript keywords with WORD__
2. rename temporary variables
 */
pub fn rename(name: &impl fmt::Display) -> String {
    let name_str = format!("{}", name);
    match name_str.as_str() {
        "new" => "new__".to_string(),
        "default" => "default__".to_string(),
        "for" => "for__".to_string(),
        _ => {
            if name_str.starts_with("%#") {
                // replace temporaries
                format!("temp${}", name_str.split_at(2).1)
            } else if name_str.contains("#") {
                // normalize shadowed variable names
                name_str.replace("#", "__")
            } else {
                name_str
            }
        }
    }
}

pub fn generate_index(package_name: &String, modules: &Vec<&ModuleIdent>) -> (String, String) {
    let filename = format!("{}/index.ts", package_name);
    let exports = modules
        .iter()
        .map(|mi| {
            format!(
                "export * as {}$_ from './{}';\n",
                mi.value.module, mi.value.module
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let imports = modules
        .iter()
        .map(|mi| {
            format!(
                "import * as {}$_ from './{}';\n",
                mi.value.module, mi.value.module
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let loads = modules
        .iter()
        .map(|mi| format!("  {}$_.loadParsers(repo);", mi.value.module))
        .join("\n");

    let content = format!(
        r###"
import {{ AptosParserRepo }} from "@manahippo/move-to-ts";
{}
{}

export function loadParsers(repo: AptosParserRepo) {{
{}
}}

export function getPackageRepo(): AptosParserRepo {{
  const repo = new AptosParserRepo();
  loadParsers(repo);
  repo.addDefaultParsers();
  return repo;
}}
"###,
        imports, exports, loads
    );

    (filename, content)
}

pub fn generate_topmost_index(packages: &Vec<&String>) -> (String, String) {
    let filename = "index.ts".to_string();
    let exports = packages
        .iter()
        .map(|package_name| format!("export * as {} from './{}';\n", package_name, package_name))
        .collect::<Vec<_>>()
        .join("");

    let imports = packages
        .iter()
        .map(|package_name| format!("import * as {} from './{}';\n", package_name, package_name))
        .collect::<Vec<_>>()
        .join("");

    let loads = packages
        .iter()
        .map(|p| format!("  {}.loadParsers(repo);", p))
        .join("\n");

    let content = format!(
        r###"
import {{ AptosParserRepo }} from "@manahippo/move-to-ts";
{}
{}

export function getProjectRepo(): AptosParserRepo {{
  const repo = new AptosParserRepo();
{}
  repo.addDefaultParsers();
  return repo;
}}
"###,
        imports, exports, loads
    );

    (filename, content)
}

pub fn get_table_helper_decl() -> String {
    r###"
export class TypedTable<K, V> {
  static buildFromField<K, V>(table: Table, field: FieldDeclType): TypedTable<K, V> {
    const tag = field.typeTag;
    if (!(tag instanceof StructTag)) {
      throw new Error();
    }
    if (tag.getParamlessName() !== '0x1::Table::Table') {
      throw new Error();
    }
    if (tag.typeParams.length !== 2) {
      throw new Error();
    }
    const [keyTag, valueTag] = tag.typeParams;
    return new TypedTable<K, V>(table, keyTag, valueTag);
  }

  constructor(
    public table: Table,
    public keyTag: TypeTag,
    public valueTag: TypeTag
  ) {
  }

  async loadEntryRaw(client: AptosClient, key: K): Promise<any> {
    return await client.getTableItem(this.table.handle.value.toString(), {
      key_type: $.getTypeTagFullname(this.keyTag),
      value_type: $.getTypeTagFullname(this.valueTag),
      key: $.moveValueToOpenApiObject(key, this.keyTag),
    });
  }

  async loadEntry(client: AptosClient, repo: AptosParserRepo, key: K): Promise<V> {
    const rawVal = await this.loadEntryRaw(client, key);
    return repo.parse(rawVal.data, this.valueTag);
  }
}
"###
    .to_string()
}

pub fn get_iterable_table_helper_decl() -> String {
    r###"
export class TypedIterableTable<K, V> {
  static buildFromField<K, V>(table: IterableTable, field: FieldDeclType): TypedIterableTable<K, V> {
    const tag = field.typeTag;
    if (!(tag instanceof StructTag)) {
      throw new Error();
    }
    if (tag.getParamlessName() !== '0x1::IterableTable::IterableTable') {
      throw new Error();
    }
    if (tag.typeParams.length !== 2) {
      throw new Error();
    }
    const [keyTag, valueTag] = tag.typeParams;
    return new TypedIterableTable<K, V>(table, keyTag, valueTag);
  }

  iterValueTag: StructTag;
  constructor(
    public table: IterableTable,
    public keyTag: TypeTag,
    public valueTag: TypeTag
  ) {
    this.iterValueTag = new StructTag(moduleAddress, moduleName, "IterableValue", [keyTag, valueTag])
  }

  async loadEntryRaw(client: AptosClient, key: K): Promise<any> {
    return await client.getTableItem(this.table.inner.handle.value.toString(), {
      key_type: $.getTypeTagFullname(this.keyTag),
      value_type: $.getTypeTagFullname(this.iterValueTag),
      key: $.moveValueToOpenApiObject(key, this.keyTag),
    });
  }

  async loadEntry(client: AptosClient, repo: AptosParserRepo, key: K): Promise<IterableValue> {
    const rawVal = await this.loadEntryRaw(client, key);
    return repo.parse(rawVal.data, this.iterValueTag) as IterableValue;
  }

  async fetchAll(client: AptosClient, repo: AptosParserRepo): Promise<[K, V][]> {
    const result: [K, V][] = [];
    const cache = new $.DummyCache();
    let next = this.table.head;
    while(next && Std.Option.is_some$(next, cache, [this.keyTag])) {
      const key = Std.Option.borrow$(next, cache, [this.keyTag]) as K;
      const iterVal = await this.loadEntry(client, repo, key);
      const value = iterVal.val as V;
      result.push([key, value]);
      next = iterVal.next;
    }
    return result;
  }
}
"###
        .to_string()
}
