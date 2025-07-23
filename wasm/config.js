export function defaultConfig() {
  return {
    debug: false,
    tab_size: 4,
    complement_alias: true,
    trim_bind_param: false,
    keyword_case: "lower",
    identifier_case: "lower",
    max_char_per_line: 50,
    complement_outer_keyword: true,
    complement_column_as_keyword: true,
    remove_table_as_keyword: true,
    remove_redundant_nest: true,
    complement_sql_id: false,
    convert_double_colon_cast: true,
    unify_not_equal: true,
    indent_tab: true,
    use_parser_error_recovery: true,
  };
}

// すべてのフォーム要素に値を適用する
export function setConfig(config) {
  Object.entries(config).forEach(([key, value]) => {
    const el = document.getElementById(key);
    if (!el) return;

    if (typeof value === "boolean") {
      el.checked = value;
    } else if (typeof value === "number") {
      el.value = value;
    } else {
      const options = el.options;
      for (let i = 0; i < options.length; i++) {
        if (options[i].value === value) {
          el.selectedIndex = i;
          break;
        }
      }
    }
  });
}

export const applyDefaultConfig = () => setConfig(defaultConfig());

export const selectAllConfig = () =>
  setConfig({
    complement_alias: true,
    trim_bind_param: true,
    complement_outer_keyword: true,
    complement_column_as_keyword: true,
    remove_table_as_keyword: true,
    remove_redundant_nest: true,
    complement_sql_id: true,
    convert_double_colon_cast: true,
    unify_not_equal: true,
  });

export const clearAllConfig = () =>
  setConfig({
    complement_alias: false,
    trim_bind_param: false,
    complement_outer_keyword: false,
    complement_column_as_keyword: false,
    remove_table_as_keyword: false,
    remove_redundant_nest: false,
    complement_sql_id: false,
    convert_double_colon_cast: false,
    unify_not_equal: false,
  });

export function buildConfigJson() {
  const num = (id) => parseInt(document.getElementById(id).value);
  const bool = (id) => document.getElementById(id).checked;
  const sel = (id) => document.getElementById(id).selectedOptions[0].value;

  return JSON.stringify({
    debug: false,
    tab_size: num("tab_size"),
    complement_alias: bool("complement_alias"),
    trim_bind_param: bool("trim_bind_param"),
    keyword_case: sel("keyword_case"),
    identifier_case: sel("identifier_case"),
    max_char_per_line: num("max_char_per_line"),
    complement_outer_keyword: bool("complement_outer_keyword"),
    complement_column_as_keyword: bool("complement_column_as_keyword"),
    remove_table_as_keyword: bool("remove_table_as_keyword"),
    remove_redundant_nest: bool("remove_redundant_nest"),
    complement_sql_id: bool("complement_sql_id"),
    convert_double_colon_cast: bool("convert_double_colon_cast"),
    unify_not_equal: bool("unify_not_equal"),
    indent_tab: bool("indent_tab"),
    use_parser_error_recovery: bool("use_parser_error_recovery"),
  });
}
