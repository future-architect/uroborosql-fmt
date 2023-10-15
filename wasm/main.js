let sql_value = [
  "select tbl.a, tbl.b as b",
  "from table_name tbl",
  "where a = /* param   */'a'",
].join("\n");

let src_editor = null;
let dst_editor = null;

// monaco editor
require.config({
  paths: {
    vs: "https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.43.0/min/vs",
  },
});
require(["vs/editor/editor.main"], function () {
  src_editor = monaco.editor.create(document.getElementById("src_editor"), {
    value: sql_value,
    language: "sql",
  });

  src_editor.onDidChangeModelContent(() => {
    sql_value = src_editor.getValue();
  });

  dst_editor = monaco.editor.create(document.getElementById("dst_editor"), {
    value: "",
    language: "sql",
    readOnly: true,
  });
});

Module = {
  onRuntimeInitialized: initialize,
  onAborted: abort,
};

// ローカルストレージから設定を復元
const storedConfig = localStorage.getItem("config");
if (storedConfig) {
  const config = JSON.parse(storedConfig);
  Object.keys(config).forEach((key) => {
    const element = document.getElementById(key);

    if (!element) return;

    const value = config[key];

    if (typeof value === "boolean") {
      element.checked = value;
    } else if (typeof value === "number") {
      element.value = value;
    } else {
      let options = element.options;
      for (let i = 0; i < options.length; i++) {
        if (options[i].value === value) {
          element.selectedIndex = i;
          break;
        }
      }
    }
  });
}

// wasmの初期化が終了した際の処理
function initialize() {
  function formatSql() {
    if (!src_editor || !dst_editor) {
      console.log("editors have not been loaded.");
      return;
    }

    const target = sql_value;

    // tab_size
    const tab_size = parseInt(document.getElementById("tab_size").value);

    // complement_alias
    const complement_alias =
      document.getElementById("complement_alias").checked;

    // trim_bind_param
    const trim_bind_param = document.getElementById("trim_bind_param").checked;

    // keyword_case
    const keyword_case =
      document.getElementById("keyword_case").selectedOptions[0].value;

    // identifier_case
    const identifier_case =
      document.getElementById("identifier_case").selectedOptions[0].value;

    // max_char_per_line
    const max_char_per_line = parseInt(
      document.getElementById("max_char_per_line").value
    );

    // complement_outer_keyword
    const compement_outer_keyword = document.getElementById(
      "complement_outer_keyword"
    ).checked;

    // complement_column_as_keyword
    const complement_column_as_keyword = document.getElementById(
      "complement_column_as_keyword"
    ).checked;

    // remove_table_as_keyword
    const remove_table_as_keyword = document.getElementById(
      "remove_table_as_keyword"
    ).checked;

    // remove_redundant_nest
    const remove_redundant_nest = document.getElementById(
      "remove_redundant_nest"
    ).checked;

    // complement_sql_id
    const complement_sql_id =
      document.getElementById("complement_sql_id").checked;

    // convert_double_colon_cast
    const convert_double_colon_cast = document.getElementById(
      "convert_double_colon_cast"
    ).checked;

    // unify_not_equal
    const unify_not_equal = document.getElementById("unify_not_equal").checked;

    const config = {
      debug: false,
      tab_size: tab_size,
      complement_alias: complement_alias,
      trim_bind_param: trim_bind_param,
      keyword_case: keyword_case,
      identifier_case: identifier_case,
      max_char_per_line: max_char_per_line,
      complement_outer_keyword: compement_outer_keyword,
      complement_column_as_keyword: complement_column_as_keyword,
      remove_table_as_keyword: remove_table_as_keyword,
      remove_redundant_nest: remove_redundant_nest,
      complement_sql_id: complement_sql_id,
      convert_double_colon_cast: convert_double_colon_cast,
      unify_not_equal: unify_not_equal,
    };

    const config_str = JSON.stringify(config);

    // タイマースタート
    const startTime = performance.now();

    const ptr = ccall(
      "format_sql",
      "number",
      ["string", "string"],
      [target, config_str]
    );

    // タイマーストップ
    const endTime = performance.now();
    // 何ミリ秒かかったかを表示する
    console.log("format complete: " + (endTime - startTime) + "ms");

    // Module.UTF8ToString() でポインタを js の string に変換
    const res = UTF8ToString(ptr);

    // Rust 側で確保したフォーマット文字列の所有権を返す
    ccall("free_format_string", null, ["number"], [ptr]);

    dst_editor.setValue(res);

    src_editor.updateOptions({ tabSize: tab_size });
    dst_editor.updateOptions({ tabSize: tab_size });

    // 設定をjson形式でローカルストレージに保存
    const encodedConfig = JSON.stringify(config);
    localStorage.setItem("config", encodedConfig);
  }

  const button = document.getElementById("format");
  button.addEventListener("click", (event) => formatSql());

  document.getElementById("copy").addEventListener("click", (event) => {
    const value = dst_editor.getValue();
    navigator.clipboard.writeText(value).catch((reason) => console.log(reason));
  });

  addEventListener("keydown", (event) => {
    if (event.ctrlKey && event.shiftKey && event.code == "KeyF") {
      formatSql();
    }
  });
}

function abort(msg) {
  let alert_msg = [
    "wasmのロードに失敗しました。リロードしてください。",
    "error message:",
    msg,
  ].join("\n");

  alert(alert_msg);
}
