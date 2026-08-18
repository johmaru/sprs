# ランタイム関数

これらのシンボルはコンパイラとランタイムの内部 API です。
言語組み込みではありません。

Sprs ソースからこれらの `__` シンボルを呼ばないでください。
言語レベルの API ではありません。

ランタイム値は `{ i32 tag, i64 data }` です。
ヒープ値では、`data` はハンドル `(index:u32 << 32) | generation:u32` です。
ハンドル `0` は無効です。
Atom の `data` は intern id です。
RawPtr の `data` は素のアドレスです。

| Tag | Value |
|-----|-------|
| Integer | 0 |
| Float | 1 |
| String | 2 |
| Boolean | 3 |
| List | 4 |
| Range | 5 |
| Unit | 6 |
| (unused) | 7 |
| Struct | 8 |
| Atom | 9 |
| Label | 10 |
| Buffer | 11 |
| RawPtr | 12 |
| Int8 | 100 |
| Uint8 | 101 |
| Int16 | 102 |
| Uint16 | 103 |
| Int32 | 104 |
| Uint32 | 105 |
| Int64 | 106 |
| Uint64 | 107 |
| Float16 | 108 |
| Float32 | 109 |
| Float64 | 110 |

| Function Name   | Description                          |
|-----------------|--------------------------------------|
| __list_new | 新しいリストを作る |
| __list_get | 添字でリストから要素を取る |
| __list_push | リスト末尾へ要素を追加する |
| __range_new | 新しい Range を作る |
| __println | 値をコンソールへ表示する |
| __strlen | 文字列長を取る |
| __malloc | メモリを割り当てる |
| __drop | 値を drop する |
| __clone | 値をクローンする |
| __panic | パニック状況を扱う |
| __buffer_new | Buffer を割り当てる |
| __buffer_len | Buffer 長 |
| __buffer_get | Buffer のバイト読み取り |
| __buffer_set | Buffer のバイト書き込み |
| __buffer_exist | Buffer の生存検査 |
| __buffer_into_raw | Buffer のバイトを素のアドレスへムーブする |
| __raw_free | __buffer_into_raw 由来のアドレスを解放する |
| __atom_from_bytes | バイトから静的な名前を intern し、その atom id を返す。 |
| __atom_from_string | String スロットの内容を intern して atom id にする。 |
| __atom_name | atom id の名前を新しい String スロットとして返す。 |
| __atom_eq | 2 つの atom id を比較する。等しければ 1、そうでなければ 0。 |
| __label_new | 名前バイトとランタイムペイロード 1 つからラベルスロットを作る。 |
| __label_new_from_string | 名前が String スロットハンドル由来のラベルを作る。 |
| __label_name_eq | ラベル名を静的バイト列と比較する。 |
| __label_names_equal | 2 つのラベルハンドルを名前で比較する。 |
| __label_payload | ラベルからクローンしたペイロードを返す。非ラベル → Unit。 |
| __label_name | ラベル名を新しい String スロットとして返す。 |
| __label_is_error | 値が `"error"` という名前の Label なら 1、そうでなければ 0 を返す。 |
| __error_label_from_str | UTF-8 バイトから String ペイロード付きの `{:error, msg}` を作る。 |
| __error_message_from_label | エラーラベルの理由を String スロットとして返す。 |
| __value_to_string | ラベル補間用にランタイム値を String スロットへ変換する。 |
| __string_new | バイトポインタと長さから String スロットを割り当てる。 |
| __string_from_cstr | C 文字列ポインタから String スロットを割り当てる。 |
| __string_concat | 2 つの String スロットを新しい String スロットへ連結する。 |
| __string_eq | 2 つの String スロットハンドルを内容で比較する。 |
| __struct_new | `size` バイトを持つ構造体スロットを割り当てる。 |
| __struct_borrow | フィールドアクセス用に生の構造体ポインタを借用する。 |
| __struct_track_value | フィールド値を登録し、構造体の drop/clone が所有するようにする。 |
| __sprs_set_output | `__println` 用のホスト出力コールバックを登録する。未登録なら `__println` は `eprintln!` を使う。コンパイラの `get_runtime_fn` はこのシンボルを宣言しない。 |
