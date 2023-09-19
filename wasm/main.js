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

// wasmの初期化が終了した際の処理
function initialize() {
  function formatSql() {
    if (!src_editor || !dst_editor) {
      console.log("editors have not been loaded.");
      return;
    }

    const target = sql_value;

    const tab_size = parseInt(document.getElementById("tab_size").value);
    const complement_alias =
      document.getElementById("complement_alias").checked;
    const trim_bind_param = document.getElementById("trim_bind_param").checked;
    const keyword_case =
      document.getElementById("keyword_case").selectedOptions[0].value;
    const identifier_case =
      document.getElementById("identifier_case").selectedOptions[0].value;

    const config = {
      debug: false,
      tab_size: tab_size,
      complement_alias: complement_alias,
      trim_bind_param: trim_bind_param,
      keyword_case: keyword_case,
      identifier_case: identifier_case,
      max_char_per_line: 50,
      complement_outer_keyword: true,
      complement_column_as_keyword: true,
      remove_table_as_keyword: true,
      remove_redundant_nest: true,
      complement_sql_id: true,
      convert_double_colon_cast: false,
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
