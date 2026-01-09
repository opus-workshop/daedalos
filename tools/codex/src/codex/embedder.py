"""Embedding generation for semantic search."""

import hashlib
import json
import math
import re
import subprocess
from collections import Counter
from pathlib import Path
from typing import Dict, List, Optional, Tuple


class TFIDFEmbedder:
    """TF-IDF based embedder as fallback when Ollama isn't available."""

    def __init__(self):
        self.vocabulary: Dict[str, int] = {}
        self.idf: Dict[str, float] = {}
        self.vocab_size = 0

    def to_dict(self) -> dict:
        """Serialize the model to a dictionary."""
        return {
            "vocabulary": self.vocabulary,
            "idf": self.idf,
            "vocab_size": self.vocab_size,
        }

    @classmethod
    def from_dict(cls, data: dict) -> "TFIDFEmbedder":
        """Deserialize from a dictionary."""
        embedder = cls()
        embedder.vocabulary = data.get("vocabulary", {})
        embedder.idf = data.get("idf", {})
        embedder.vocab_size = data.get("vocab_size", 0)
        return embedder

    def _tokenize(self, text: str) -> List[str]:
        """Tokenize text into words."""
        # Lowercase, split on non-alphanumeric, filter short tokens
        text = text.lower()
        tokens = re.findall(r'\b[a-z_][a-z0-9_]*\b', text)
        return [t for t in tokens if len(t) > 2]

    def fit(self, documents: List[str]):
        """Build vocabulary and IDF from documents."""
        # Build vocabulary
        all_tokens = set()
        doc_freqs: Dict[str, int] = Counter()

        for doc in documents:
            tokens = set(self._tokenize(doc))
            all_tokens.update(tokens)
            for token in tokens:
                doc_freqs[token] += 1

        # Create vocabulary mapping
        self.vocabulary = {token: i for i, token in enumerate(sorted(all_tokens))}
        self.vocab_size = len(self.vocabulary)

        # Calculate IDF
        n_docs = len(documents)
        for token, freq in doc_freqs.items():
            self.idf[token] = math.log(n_docs / (1 + freq))

    def embed(self, text: str) -> List[float]:
        """Generate TF-IDF embedding for text."""
        if self.vocab_size == 0:
            # Not fitted yet, return zero vector
            return [0.0] * 100

        tokens = self._tokenize(text)
        tf = Counter(tokens)

        # Create sparse vector
        vector = [0.0] * self.vocab_size
        for token, count in tf.items():
            if token in self.vocabulary:
                idx = self.vocabulary[token]
                tfidf = count * self.idf.get(token, 0)
                vector[idx] = tfidf

        # Normalize
        norm = math.sqrt(sum(v * v for v in vector))
        if norm > 0:
            vector = [v / norm for v in vector]

        return vector

    def embed_batch(self, texts: List[str]) -> List[List[float]]:
        """Embed multiple texts."""
        return [self.embed(t) for t in texts]


class OllamaEmbedder:
    """Ollama-based embedder using nomic-embed-text."""

    def __init__(self, model: str = "nomic-embed-text"):
        self.model = model
        self._available: Optional[bool] = None

    def is_available(self) -> bool:
        """Check if Ollama is available and model is pulled."""
        if self._available is not None:
            return self._available

        try:
            result = subprocess.run(
                ["ollama", "list"],
                capture_output=True,
                text=True,
                timeout=5
            )
            self._available = result.returncode == 0
            return self._available
        except (FileNotFoundError, subprocess.TimeoutExpired):
            self._available = False
            return False

    def ensure_model(self) -> bool:
        """Ensure the embedding model is available."""
        if not self.is_available():
            return False

        try:
            result = subprocess.run(
                ["ollama", "list"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if self.model not in result.stdout:
                print(f"Pulling embedding model {self.model}...")
                subprocess.run(
                    ["ollama", "pull", self.model],
                    timeout=300  # 5 minutes for model download
                )
            return True
        except (subprocess.TimeoutExpired, subprocess.CalledProcessError):
            return False

    def embed(self, text: str) -> List[float]:
        """Generate embedding using Ollama."""
        try:
            # Use ollama's embeddings API
            result = subprocess.run(
                ["ollama", "embeddings", self.model],
                input=json.dumps({"prompt": text[:2000]}),
                capture_output=True,
                text=True,
                timeout=30
            )
            if result.returncode == 0:
                data = json.loads(result.stdout)
                return data.get("embedding", [])
        except (subprocess.TimeoutExpired, json.JSONDecodeError, KeyError):
            pass

        return []

    def embed_batch(self, texts: List[str]) -> List[List[float]]:
        """Embed multiple texts."""
        return [self.embed(t) for t in texts]


class HybridEmbedder:
    """Hybrid embedder that uses Ollama when available, TF-IDF as fallback."""

    def __init__(self, model: str = "nomic-embed-text"):
        self.ollama = OllamaEmbedder(model)
        self.tfidf = TFIDFEmbedder()
        self._use_ollama: Optional[bool] = None
        self._fitted = False

    @property
    def use_ollama(self) -> bool:
        """Determine which backend to use."""
        if self._use_ollama is None:
            self._use_ollama = self.ollama.is_available() and self.ollama.ensure_model()
        return self._use_ollama

    @property
    def backend_name(self) -> str:
        """Return the name of the active backend."""
        return "ollama" if self.use_ollama else "tfidf"

    def fit(self, documents: List[str]):
        """Fit TF-IDF model (only used when Ollama unavailable)."""
        if not self.use_ollama:
            self.tfidf.fit(documents)
            self._fitted = True

    def embed(self, text: str) -> List[float]:
        """Generate embedding using best available method."""
        if self.use_ollama:
            embedding = self.ollama.embed(text)
            if embedding:
                return embedding
            # Fallback if Ollama fails
            self._use_ollama = False

        return self.tfidf.embed(text)

    def embed_batch(self, texts: List[str]) -> List[List[float]]:
        """Embed multiple texts."""
        if self.use_ollama:
            embeddings = []
            for text in texts:
                emb = self.embed(text)
                embeddings.append(emb)
            return embeddings
        return self.tfidf.embed_batch(texts)

    def to_dict(self) -> dict:
        """Serialize the embedder state."""
        return {
            "backend": self.backend_name,
            "tfidf": self.tfidf.to_dict() if not self.use_ollama else None,
        }

    def load_state(self, data: dict):
        """Load embedder state from serialized dict."""
        if data.get("tfidf"):
            self.tfidf = TFIDFEmbedder.from_dict(data["tfidf"])
            self._fitted = True


# Default embedder
def get_embedder(model: str = "nomic-embed-text") -> HybridEmbedder:
    """Get the default embedder instance."""
    return HybridEmbedder(model)
