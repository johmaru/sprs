# モジュール

* マクロ定義のための `#define`

現状この言語には次があります。

* OS 検出のための `#define Windows` または `#define Linux`
* モジュール定義のための `pkg`
* モジュール import のための `import`

例:

```sprs

import test;
#define Windows

       fn main() {
          var x = test.test();
          var y = [];
          var z = 20;
          var alpha = "test";
          var beta = true;
          @println(x);
          @list_push(y, z);
          @list_push(y, alpha);
          @println(y[1]);

             var result = (x + 10) * 2;
             @println(result);
             var i = @cast(0, i8);
               while i <= 5 {
                   @println(i);
                   i = i + 1;
               }

             var m = 10 % 3;
             @println(m);
       }

```

```sprs

pkg test;

 fn test() {
           var a = 5 - 1;
           var b = 10;
           var c = "hello" + " world";
           @println(c);

           if a == 3 {
               return a;
           }

           if a != 3 {
               return a++;
           } else {
               return a + 2;
           }

           return b;
      }
```
