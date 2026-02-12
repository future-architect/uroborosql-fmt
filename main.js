import init, {
  format_sql_for_wasm as format_sql,
} from "./uroborosql_fmt_wasm.js";
import {
  applyDefaultConfig,
  buildConfigJson,
  clearAllConfig,
  selectAllConfig,
  setConfig,
} from "./config.js";

let src_editor = null;
let dst_editor = null;

// 初期表示用のサンプル SQL
let sql_value = [
  "select tbl.a, tbl.b as b",
  "from table_name tbl",
  "where a = /* param   */'a'",
].join("\n");

// Monaco Editor を AMD loader で非同期ロードする
function loadMonaco() {
  return new Promise((resolve) => {
    require.config({
      paths: {
        vs:
          "https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.43.0/min/vs",
      },
    });
    require(["vs/editor/editor.main"], () => {
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

      resolve();
    });
  });
}

function formatSql() {
  if (!src_editor || !dst_editor) {
    console.log("editors are not ready.");
    return;
  }
  const target = sql_value;
  const config_str = buildConfigJson();

  // 設定をローカルストレージに保存
  localStorage.setItem("config", config_str);

  try {
    const startTime = performance.now();

    // wasm-bindgen 経由のフォーマット
    const res = format_sql(target, config_str);

    const endTime = performance.now();
    console.log(`format complete: ${endTime - startTime}ms`);

    dst_editor.setValue(res);
    document.getElementById("error_msg").innerText = "";

    const { tab_size } = JSON.parse(config_str);
    src_editor.updateOptions({ tabSize: tab_size });
    dst_editor.updateOptions({ tabSize: tab_size });
  } catch (e) {
    dst_editor.setValue("");
    document.getElementById("error_msg").innerText = e.message ?? String(e);
  }
}

// ボタン等のイベント登録
function registerUIEvents() {
  // default button
  document.getElementById("default").addEventListener(
    "click",
    applyDefaultConfig,
  );
  // select all button
  document.getElementById("select_all").addEventListener(
    "click",
    selectAllConfig,
  );
  // clear all button
  document.getElementById("clear_all").addEventListener(
    "click",
    clearAllConfig,
  );

  // format button
  document.getElementById("format").addEventListener(
    "click",
    () => formatSql(),
  );

  // copy button
  document.getElementById("copy").addEventListener("click", () => {
    const value = dst_editor.getValue();
    navigator.clipboard.writeText(value).catch((reason) => console.log(reason));
  });

  // Ctrl + Shift + F shortcut
  addEventListener("keydown", (event) => {
    if (event.ctrlKey && event.shiftKey && event.code === "KeyF") {
      formatSql();
    }
  });
}

async function main() {
  // wasm と Monaco を初期化
  await Promise.all([init(), loadMonaco()]);

  // 設定をローカルストレージから復元
  const storedConfig = localStorage.getItem("config");
  if (storedConfig) {
    setConfig(JSON.parse(storedConfig));
  } else {
    applyDefaultConfig();
  }

  registerUIEvents();
}

main().catch((e) => {
  alert(["wasm load failed. reload.", "error message:", e].join("\n"));
});
