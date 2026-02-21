//
// Created by user on 5/9/25.
//

#ifndef VAULTWOLF_VAULT_CONTRACT_H
#define VAULTWOLF_VAULT_CONTRACT_H

#include "Contract.h"

class ContractSamples {
public:
    static Contract IBMBond();
    static Contract IBKRStk();
    static Contract HKStk();
    static Contract EurGbpFx();
    static Contract Index();
    static Contract CFD();
    static Contract USStockCFD();
    static Contract EuropeanStockCFD();
    static Contract CashCFD();
    static Contract EuropeanStock();
    static Contract OptionAtIse();
    static Contract USStock();
    static Contract etf();
    static Contract USStockAtSmart();
    static Contract IBMUSStockAtSmart();
    static Contract USStockWithPrimaryExch();
    static Contract BondWithCusip();
    static Contract Bond();
    static Contract MutualFund();
    static Contract Commodity();
    static Contract USOptionContract();
    static Contract OptionAtBox();
    static Contract OptionWithTradingClass();
    static Contract OptionWithLocalSymbol();
    static Contract DutchWarrant();
    static Contract SimpleFuture();
    static Contract FutureWithLocalSymbol();
    static Contract FutureWithMultiplier();
    static Contract WrongContract();
    static Contract FuturesOnOptions();
    static Contract ByISIN();
    static Contract ByConId();
    static Contract OptionForQuery();
    static Contract StockComboContract();
    static Contract FutureComboContract();
    static Contract SmartFutureComboContract();
    static Contract OptionComboContract();
    static Contract InterCmdtyFuturesContract();
    static Contract NewsFeedForQuery();
    static Contract BTbroadtapeNewsFeed();
    static Contract BZbroadtapeNewsFeed();
    static Contract FLYbroadtapeNewsFeed();
    //static Contract MTbroadtapeNewsFeed();
    static Contract ContFut();
    static Contract ContAndExpiringFut();
    static Contract JefferiesContract();
    static Contract CSFBContract();
    static Contract Warrants();
    static Contract IBKRATSContract();
    static Contract CryptoContract();
    static Contract StockWithIPOPrice();
    static Contract ByFIGI();
    static Contract ByIssuerId();
    static Contract Fund();
};

#endif //VAULTWOLF_VAULT_CONTRACT_H
