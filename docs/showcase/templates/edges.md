@grid edges columns=3,2
---
<table>
{{#edges_rows}}
<tr>
{{#cells}}
<td width="{{edges_cell_width}}%" align="center" valign="top">
{{^filler}}
<img src="{{glyph}}" alt="" width="96" height="56"><br>
<b>{{name}}</b><br>
<sub>{{text}}</sub>
{{/filler}}
</td>
{{/cells}}
</tr>
{{/edges_rows}}
</table>
