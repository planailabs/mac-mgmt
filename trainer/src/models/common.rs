use burn::config::Config;
use burn::module::Module;
use burn::nn::{
    Dropout, DropoutConfig, Embedding, EmbeddingConfig, LayerNorm, LayerNormConfig, Linear,
    LinearConfig,
};
use burn::tensor::backend::Backend;
use burn::tensor::{Int, Tensor};

/// Configuration for the shared session encoder.
#[derive(Config, Debug)]
pub struct SessionEncoderConfig {
    /// Vocabulary size for token embeddings.
    #[config(default = 8192)]
    pub vocab_size: usize,
    /// Model hidden dimension.
    #[config(default = 128)]
    pub d_model: usize,
    /// Number of attention heads.
    #[config(default = 4)]
    pub n_heads: usize,
    /// Number of transformer layers.
    #[config(default = 2)]
    pub n_layers: usize,
    /// Feed-forward hidden dimension (typically 4x d_model).
    #[config(default = 512)]
    pub d_ff: usize,
    /// Number of distinct roles.
    #[config(default = 10)]
    pub n_roles: usize,
    /// Maximum sequence length.
    #[config(default = 512)]
    pub max_seq_len: usize,
    /// Dropout rate.
    #[config(default = 0.3)]
    pub dropout: f64,
}

/// Shared transformer encoder for processing session message sequences.
///
/// Combines token embeddings, role embeddings, and positional embeddings,
/// then passes through a stack of simplified transformer layers.
#[derive(Module, Debug)]
pub struct SessionEncoder<B: Backend> {
    pub token_embedding: Embedding<B>,
    pub role_embedding: Embedding<B>,
    pub position_embedding: Embedding<B>,
    layers: Vec<TransformerLayer<B>>,
    norm: LayerNorm<B>,
    dropout: Dropout,
}

impl SessionEncoderConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> SessionEncoder<B> {
        let token_embedding = EmbeddingConfig::new(self.vocab_size, self.d_model).init(device);
        let role_embedding = EmbeddingConfig::new(self.n_roles, self.d_model).init(device);
        let position_embedding = EmbeddingConfig::new(self.max_seq_len, self.d_model).init(device);

        let layers = (0..self.n_layers)
            .map(|_| {
                TransformerLayerConfig {
                    d_model: self.d_model,
                    n_heads: self.n_heads,
                    d_ff: self.d_ff,
                    dropout: self.dropout,
                }
                .init(device)
            })
            .collect();

        let norm = LayerNormConfig::new(self.d_model).init(device);
        let dropout = DropoutConfig::new(self.dropout).init();

        SessionEncoder {
            token_embedding,
            role_embedding,
            position_embedding,
            layers,
            norm,
            dropout,
        }
    }
}

impl<B: Backend> SessionEncoder<B> {
    /// Forward pass.
    ///
    /// - `tokens`: [batch, seq_len] token IDs
    /// - `roles`: [batch, seq_len] role IDs
    /// - `mask`: [batch, seq_len] attention mask (1.0 = attend, 0.0 = ignore)
    ///
    /// Returns: [batch, seq_len, d_model] encoded representations.
    pub fn forward(
        &self,
        tokens: Tensor<B, 2, Int>,
        roles: Tensor<B, 2, Int>,
        mask: Tensor<B, 2>,
    ) -> Tensor<B, 3> {
        let [batch, seq_len] = tokens.dims();
        let device = tokens.device();

        // Position indices
        let positions = Tensor::<B, 1, Int>::arange(0..seq_len as i64, &device)
            .unsqueeze::<2>()
            .expand([batch, seq_len]);

        // Combine embeddings
        let tok_emb = self.token_embedding.forward(tokens);
        let role_emb = self.role_embedding.forward(roles);
        let pos_emb = self.position_embedding.forward(positions);

        let mut x = tok_emb + role_emb + pos_emb;
        x = self.dropout.forward(x);

        // Apply transformer layers with mask
        for layer in &self.layers {
            x = layer.forward(x, mask.clone());
        }

        self.norm.forward(x)
    }

    /// Pool the encoder output to a single vector per batch element.
    /// Uses masked mean pooling.
    pub fn pool(&self, encoded: Tensor<B, 3>, mask: Tensor<B, 2>) -> Tensor<B, 2> {
        // mask: [batch, seq_len] → [batch, seq_len, 1]
        let mask_3d: Tensor<B, 3> = mask.clone().unsqueeze_dim::<3>(2);
        let masked = encoded * mask_3d;
        let sum: Tensor<B, 2> = masked.sum_dim(1).squeeze(1); // [batch, d_model]
        let count: Tensor<B, 2> = mask.sum_dim(1).clamp_min(1.0); // [batch, 1]
        sum / count
    }
}

/// A simplified transformer layer (pre-norm style).
#[derive(Module, Debug)]
pub struct TransformerLayer<B: Backend> {
    norm1: LayerNorm<B>,
    attn_qkv: Linear<B>,
    attn_out: Linear<B>,
    norm2: LayerNorm<B>,
    ff1: Linear<B>,
    ff2: Linear<B>,
    dropout: Dropout,
    n_heads: usize,
}

#[derive(Config, Debug)]
struct TransformerLayerConfig {
    d_model: usize,
    n_heads: usize,
    d_ff: usize,
    dropout: f64,
}

impl TransformerLayerConfig {
    fn init<B: Backend>(&self, device: &B::Device) -> TransformerLayer<B> {
        TransformerLayer {
            norm1: LayerNormConfig::new(self.d_model).init(device),
            attn_qkv: LinearConfig::new(self.d_model, self.d_model * 3).init(device),
            attn_out: LinearConfig::new(self.d_model, self.d_model).init(device),
            norm2: LayerNormConfig::new(self.d_model).init(device),
            ff1: LinearConfig::new(self.d_model, self.d_ff).init(device),
            ff2: LinearConfig::new(self.d_ff, self.d_model).init(device),
            dropout: DropoutConfig::new(self.dropout).init(),
            n_heads: self.n_heads,
        }
    }
}

impl<B: Backend> TransformerLayer<B> {
    fn forward(&self, x: Tensor<B, 3>, mask: Tensor<B, 2>) -> Tensor<B, 3> {
        // Pre-norm self-attention
        let residual = x.clone();
        let x_norm = self.norm1.forward(x);
        let attn = self.self_attention(x_norm, mask);
        let x = residual + self.dropout.forward(attn);

        // Pre-norm feed-forward
        let residual = x.clone();
        let x_norm = self.norm2.forward(x);
        let ff = self
            .ff2
            .forward(burn::tensor::activation::gelu(self.ff1.forward(x_norm)));
        residual + self.dropout.forward(ff)
    }

    fn self_attention(&self, x: Tensor<B, 3>, mask: Tensor<B, 2>) -> Tensor<B, 3> {
        let [batch, seq_len, d_model] = x.dims();
        let head_dim = d_model / self.n_heads;

        // Project to Q, K, V
        let qkv = self.attn_qkv.forward(x);
        let qkv = qkv.reshape([batch, seq_len, 3, self.n_heads, head_dim]);
        let qkv = qkv.swap_dims(2, 3); // [batch, seq_len, n_heads, 3, head_dim]

        // Split Q, K, V — slice along the "3" dimension
        let q = qkv.clone().narrow(3, 0, 1).squeeze::<4>(3); // [batch, seq_len, n_heads, head_dim]
        let k = qkv.clone().narrow(3, 1, 1).squeeze::<4>(3);
        let v = qkv.narrow(3, 2, 1).squeeze::<4>(3);

        // Transpose for attention: [batch, n_heads, seq_len, head_dim]
        let q = q.swap_dims(1, 2);
        let k = k.swap_dims(1, 2);
        let v = v.swap_dims(1, 2);

        // Scaled dot-product attention
        let scale = (head_dim as f64).sqrt();
        let scores = q.matmul(k.swap_dims(2, 3)) / scale; // [batch, n_heads, seq_len, seq_len]

        // Apply mask: [batch, seq_len] → [batch, 1, 1, seq_len]
        let mask_4d: Tensor<B, 4> = mask.unsqueeze_dim::<3>(1).unsqueeze_dim::<4>(1);
        // Set masked positions to large negative value
        let neg_inf = Tensor::<B, 4>::ones_like(&scores) * (-1e9);
        let scores = scores * mask_4d.clone() + neg_inf * (Tensor::ones_like(&mask_4d) - mask_4d);

        let attn_weights = burn::tensor::activation::softmax(scores, 3);
        let attn_output = attn_weights.matmul(v); // [batch, n_heads, seq_len, head_dim]

        // Reshape back
        let attn_output = attn_output.swap_dims(1, 2); // [batch, seq_len, n_heads, head_dim]
        let attn_output = attn_output.reshape([batch, seq_len, d_model]);

        self.attn_out.forward(attn_output)
    }
}
