import { program } from 'commander';
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram, TransactionConfirmationStrategy } from '@solana/web3.js';
import { configProject, launchToken, migrate, setClusterConfig, swap, withdraw } from './scripts';
import * as anchor from '@coral-xyz/anchor';
import { setTimeout } from 'timers/promises';

program.version('0.0.1');

programCommand('airdrop').action(async (directory, cmd) => {
  const { env, keypair, rpc } = cmd.opts();

  await setClusterConfig(env, keypair, rpc);

  const { connection, publicKey } = anchor.getProvider();

  while (true) {
    console.log('wallet balance', (await connection.getBalance(publicKey)) / LAMPORTS_PER_SOL, 'SOL');
    try {
      const wallet = Keypair.generate();
      const airdropTx = await connection.requestAirdrop(wallet.publicKey, 5 * LAMPORTS_PER_SOL);
      await connection.confirmTransaction(
        {
          signature: airdropTx
          // abortSignal: AbortSignal.timeout(30000)
        } as TransactionConfirmationStrategy,
        'finalized'
      );

      const balance = await connection.getBalance(wallet.publicKey);

      if (balance > 0) {
        console.log('new balance', wallet.publicKey.toBase58(), balance / LAMPORTS_PER_SOL, 'SOL');
        const transaction = new anchor.web3.Transaction().add(SystemProgram.transfer({ fromPubkey: wallet.publicKey, toPubkey: publicKey, lamports: Math.round(balance * 0.95) }));
        // Sign transaction, broadcast, and confirm
        const signature = await anchor.web3.sendAndConfirmTransaction(connection, transaction, [wallet], { commitment: 'processed' });
        console.log('signature', signature);
      }
    } catch (ex) {
      console.error(ex);
      await setTimeout(1000);
    }
  }
});

programCommand('config').action(async (directory, cmd) => {
  const { env, keypair, rpc } = cmd.opts();

  await setClusterConfig(env, keypair, rpc);

  await configProject();
});

programCommand('launch').action(async (directory, cmd) => {
  const { env, keypair, rpc } = cmd.opts();

  await setClusterConfig(env, keypair, rpc);

  await launchToken();
});

programCommand('swap')
  .option('-t, --token <string>', 'token address')
  .option('-a, --amount <number>', 'swap amount')
  .option('-s, --style <string>', '0: buy token, 1: sell token')
  .action(async (directory, cmd) => {
    const { env, keypair, rpc, token, amount, style } = cmd.opts();

    await setClusterConfig(env, keypair, rpc);

    if (token === undefined) {
      console.log('Error token address');
      return;
    }

    if (amount === undefined) {
      console.log('Error swap amount');
      return;
    }

    if (style === undefined) {
      console.log('Error swap style');
      return;
    }

    await swap(new PublicKey(token), amount, style);
  });

programCommand('migrate')
  .option('-t, --token <string>', 'token address')
  .action(async (directory, cmd) => {
    const { env, keypair, rpc, token } = cmd.opts();

    await setClusterConfig(env, keypair, rpc);

    if (token === undefined) {
      console.log('Error token address');
      return;
    }

    await migrate(new PublicKey(token));
  });

programCommand('withdraw')
  .option('-t, --token <string>', 'token address')
  .action(async (directory, cmd) => {
    const { env, keypair, rpc, token } = cmd.opts();

    await setClusterConfig(env, keypair, rpc);

    if (token === undefined) {
      console.log('Error token address');
      return;
    }

    await withdraw(new PublicKey(token));
  });

function programCommand(name: string) {
  return program
    .command(name)
    .option(
      //  mainnet-beta, testnet, devnet
      '-e, --env <string>',
      'Solana cluster env name',
      'devnet'
    )
    .option('-r, --rpc <string>', 'Solana cluster RPC name', 'https://api.devnet.solana.com')
    .option('-k, --keypair <string>', 'Solana wallet Keypair Path', '../key/uu.json');
}

program.parse(process.argv);

/*

yarn script config
yarn script launch
yarn script swap -t FpE6ndGpMGaPqh56vig1xDtAkGhNQSZb6hFEyyBQYd8G -a 1000000000 -s 0
yarn script migrate -t FpE6ndGpMGaPqh56vig1xDtAkGhNQSZb6hFEyyBQYd8G

*/
