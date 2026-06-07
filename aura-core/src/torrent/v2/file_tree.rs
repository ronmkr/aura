use super::super::logic::Torrent;
use super::super::metadata::V2File;

impl Torrent {
    pub fn flatten_v2_files(&self) -> Option<Vec<V2File>> {
        let tree_val = self.info.file_tree.as_ref()?;
        let mut result = Vec::new();
        Self::traverse_file_tree(tree_val, &mut Vec::new(), &mut result);
        Some(result)
    }

    fn traverse_file_tree(
        node: &serde_bencode::value::Value,
        current_path: &mut Vec<String>,
        result: &mut Vec<V2File>,
    ) {
        use serde_bencode::value::Value;

        if let Value::Dict(dict) = node {
            for (key_bytes, val) in dict {
                let key_str = String::from_utf8_lossy(key_bytes).to_string();
                if key_str.is_empty() {
                    // This node is a file. The val is a dict containing length and pieces root.
                    if let Value::Dict(props) = val {
                        let mut length = 0;
                        let mut pieces_root = None;

                        if let Some(Value::Int(l)) = props.get(b"length".as_slice()) {
                            length = *l as u64;
                        }
                        if let Some(Value::Bytes(r)) = props.get(b"pieces root".as_slice()) {
                            pieces_root = Some(r.clone());
                        }

                        result.push(V2File {
                            path: current_path.clone(),
                            length,
                            pieces_root,
                        });
                    }
                } else {
                    current_path.push(key_str);
                    Self::traverse_file_tree(val, current_path, result);
                    current_path.pop();
                }
            }
        }
    }

    pub fn get_all_pieces_roots(&self) -> Vec<[u8; 32]> {
        let mut roots = Vec::new();
        if let Some(files) = self.flatten_v2_files() {
            for file in files {
                if let Some(root) = file.pieces_root {
                    if root.len() == 32 {
                        let mut h = [0u8; 32];
                        h.copy_from_slice(&root);
                        roots.push(h);
                    }
                }
            }
        }
        roots
    }

    pub fn get_pieces_root_for_piece(&self, piece_index: usize) -> Option<[u8; 32]> {
        if self.info.meta_version != Some(2) {
            return None;
        }

        let piece_len = self.info.piece_length as usize;
        let files = self.flatten_v2_files()?;

        let mut current_piece_offset = 0;
        for file in files {
            let file_pieces = if file.length == 0 {
                0
            } else {
                (file.length as usize).div_ceil(piece_len)
            };

            if piece_index >= current_piece_offset
                && piece_index < current_piece_offset + file_pieces
            {
                return file.pieces_root.as_ref().and_then(|r| {
                    if r.len() == 32 {
                        let mut h = [0u8; 32];
                        h.copy_from_slice(r);
                        Some(h)
                    } else {
                        None
                    }
                });
            }
            current_piece_offset += file_pieces;
        }
        None
    }
}
