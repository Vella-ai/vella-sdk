export * from './index';

import { CustomTokenizerInner, PaddingParams, TruncationParams } from './index';

export class CustomTokenizer extends CustomTokenizerInner {
  /**
   * Creates a new custom tokenizer.
   */
  constructor(
    dictionary: string,
    padding?: PaddingParams,
    truncation?: TruncationParams
  ) /*throws*/ {
    super(dictionary, padding, truncation);
  }
}
