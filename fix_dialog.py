import pathlib
import traceback

try:
    f = pathlib.Path(r"C:\GitHub\cc-switch\src-tauri\src\lib.rs")
    content = f.read_text(encoding="utf-8")
    
    # Replace Chinese causes line
    old1 = '\u6570\u636e\u5e93\u7248\u672c\u8fc7\u65b0\u3001\u6587\u4ef6\u635f\u574f'
    new1 = '\u6587\u4ef6\u635f\u574f'
    c1 = content.count(old1)
    
    if c1 > 0:
        content = content.replace(old1, new1)
    
    # Remove Chinese "2) upgrade" line
    old2 = '            2) \u5982\u679c\u63d0\u793a\u201c\u6570\u636e\u5e93\u7248\u672c\u8fc7\u65b0\u201d\uff0c\u8bf7\u5347\u7ea7\u5230\u66f4\u65b0\u7248\u672c\\n'
    c2 = content.count(old2)
    
    if c2 > 0:
        content = content.replace(old2, '')
    
    # Renumber Chinese 3) -> 2)
    old3 = '            3) \u5982\u679c\u521a\u5347\u7ea7\u51fa\u73b0\u5f02\u5e38'
    new3 = '            2) \u5982\u679c\u521a\u5347\u7ea7\u51fa\u73b0\u5f02\u5e38'
    c3 = content.count(old3)
    
    if c3 > 0:
        content = content.replace(old3, new3)
    
    # English causes
    old4 = 'newer database version, corrupted file'
    new4 = 'corrupted file'
    c4 = content.count(old4)
    
    if c4 > 0:
        content = content.replace(old4, new4)
    
    # Remove English "2) If you see" line - find and remove entire line
    old5 = '            2) If you see \u201cdatabase version is newer\u201d, please upgrade CC Switch\\n\\\n'
    c5 = content.count(old5)
    
    if c5 > 0:
        content = content.replace(old5, '')
    else:
        # Try without the unicode quotes
        idx = content.find('please upgrade CC Switch')
        if idx >= 0:
            # Find the start of this line
            line_start = content.rfind('\n', 0, idx)
            # Find the end of this line (the \n\ at the end)
            line_end = content.find('\\n\\', idx)
            if line_end >= 0:
                line_end += 3  # include \n\
                old_line = content[line_start:line_end]
                content = content[:line_start] + content[line_end:]
                c5 = 1
    
    # Renumber English 3) -> 2)
    old6 = '            3) If this happened right after upgrading'
    new6 = '            2) If this happened right after upgrading'
    c6 = content.count(old6)
    
    if c6 > 0:
        content = content.replace(old6, new6)
    
    f.write_text(content, encoding="utf-8")
    
    result = f"Chinese causes: {c1} replacements\n"
    result += f"Chinese upgrade line: {c2} removals\n"
    result += f"Chinese renumber: {c3} replacements\n"
    result += f"English causes: {c4} replacements\n"
    result += f"English upgrade line: {c5} removals\n"
    result += f"English renumber: {c6} replacements\n"
    result += "SUCCESS"
    
    pathlib.Path(r"C:\GitHub\cc-switch\dialog_result.txt").write_text(result, encoding="utf-8")

except Exception as e:
    tb = traceback.format_exc()
    pathlib.Path(r"C:\GitHub\cc-switch\dialog_result.txt").write_text(f"ERROR: {e}\n{tb}", encoding="utf-8")
