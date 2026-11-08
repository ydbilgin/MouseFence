@grid features columns=3,2
---
<table>
{{#features_rows}}
<tr>
{{#cells}}
<td width="{{features_cell_width}}%" valign="top">
{{^filler}}
<b>{{name}}</b><br>
<sub>{{text}}</sub>
{{/filler}}
</td>
{{/cells}}
</tr>
{{/features_rows}}
</table>
